use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;

use cid::multihash::MultihashDigest;
use cid::Cid;
use fil_actor_account::State as AccountState;
use fil_actor_eam::EthAddress;
use fil_actor_evm::interpreter::system::StateKamt;
use fil_actor_evm::interpreter::{StatusCode, U256};
use fil_actor_init::State as InitState;
use fil_actor_reward::State as RewardState;
use fil_actor_system::State as SystemState;
use fil_actors_runtime::runtime::builtins::Type;
use fil_actors_runtime::runtime::EMPTY_ARR_CID;
use fil_actors_runtime::{
    ActorError, AsActorError, BURNT_FUNDS_ACTOR_ADDR, EAM_ACTOR_ADDR, EAM_ACTOR_ID,
    INIT_ACTOR_ADDR, REWARD_ACTOR_ADDR, SYSTEM_ACTOR_ADDR,
};
use fvm_ipld_blockstore::{Block, Blockstore};
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::CborStore;
use fvm_ipld_hamt::Hamt;
use fvm_ipld_kamt::Config as KamtConfig;
use fvm_shared::address::{Address, Payload};
use fvm_shared::crypto::hash::SupportedHashes;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::sector::StoragePower;
use fvm_shared::{HAMT_BIT_WIDTH, IPLD_RAW};
use multihash::{Code, MultihashGeneric};
use num_traits::Zero;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::evm_state::State as EvmState;
use crate::util::u256_to_bytes;

lazy_static::lazy_static! {
    // The Solidity compiler creates contiguous array item keys.
    // To prevent the tree from going very deep we use extensions,
    // which the Kamt supports and does in all cases.
    //
    // There are maximum 32 levels in the tree with the default bit width of 8.
    // The top few levels will have a higher level of overlap in their hashes.
    // Intuitively these levels should be used for routing, not storing data.
    //
    // The only exception to this is the top level variables in the contract
    // which solidity puts in the first few slots. There having to do extra
    // lookups is burdensome, and they will always be accessed even for arrays
    // because that's where the array length is stored.
    //
    // However, for Solidity, the size of the KV pairs is 2x256, which is
    // comparable to a size of a CID pointer plus extension metadata.
    // We can keep the root small either by force-pushing data down,
    // or by not allowing many KV pairs in a slot.
    //
    // The following values have been set by looking at how the charts evolved
    // with the test contract. They might not be the best for other contracts.
    pub static ref KAMT_CONFIG: KamtConfig = KamtConfig {
        min_data_depth: 0,
        bit_width: 5,
        max_array_width: 1
    };
}

#[derive(Serialize_tuple, Deserialize_tuple, Clone, PartialEq, Eq, Debug)]
pub struct Actor {
    pub code: Cid,
    pub head: Cid,
    pub nonce: u64,
    pub balance: TokenAmount,
    pub predictable_address: Option<Address>,
}

pub fn actor(
    code: Cid,
    head: Cid,
    nonce: u64,
    balance: TokenAmount,
    predictable_address: Option<Address>,
) -> Actor {
    Actor {
        code,
        head,
        nonce,
        balance,
        predictable_address,
    }
}

pub struct Mock<'bs, BS>
where
    BS: Blockstore,
{
    store: &'bs BS,
    actors: RefCell<Cid>,
    actor_codes: BTreeMap<Type, Cid>,
}

impl<'bs, BS> Mock<'bs, BS>
where
    BS: Blockstore,
{
    pub fn new(store: &'bs BS, actor_codes: BTreeMap<Type, Cid>) -> Self {
        let mut actors = Hamt::<&BS, Actor>::new_with_bit_width(store, HAMT_BIT_WIDTH);
        let actors_cid = actors.flush().unwrap();
        Self {
            store,
            actors: RefCell::new(actors_cid),
            actor_codes,
        }
    }

    pub fn mock_builtin_actor(&mut self) -> () {
        // system
        let sys_st = SystemState::new(self.store).unwrap();
        let head_cid = self
            .store
            .put_cbor(&sys_st, multihash::Code::Blake2b256)
            .unwrap();
        let faucet_total = TokenAmount::from_whole(1_000_000_000i64);
        self.set_actor(
            SYSTEM_ACTOR_ADDR,
            actor(
                self.get_actor_code(Type::System),
                head_cid,
                0,
                faucet_total,
                None,
            ),
        );

        //init
        let init_st = InitState::new(self.store, "integration-test".to_string()).unwrap();
        let head_cid = self
            .store
            .put_cbor(&init_st, multihash::Code::Blake2b256)
            .unwrap();
        let faucet_total = TokenAmount::from_whole(1_000_000_000i64);
        self.set_actor(
            INIT_ACTOR_ADDR,
            actor(
                self.get_actor_code(Type::Init),
                head_cid,
                0,
                faucet_total,
                None,
            ),
        );

        // reward
        let reward_total = TokenAmount::from_whole(1_100_000_000i64);
        let reward_head = self.put_store(&RewardState::new(StoragePower::zero()));
        self.set_actor(
            REWARD_ACTOR_ADDR,
            actor(
                self.get_actor_code(Type::Reward),
                reward_head,
                0,
                reward_total,
                None,
            ),
        );

        // Ethereum Address Manager
        self.set_actor(
            EAM_ACTOR_ADDR,
            actor(
                self.get_actor_code(Type::EAM),
                EMPTY_ARR_CID,
                0,
                TokenAmount::zero(),
                None,
            ),
        );

        // burnt funds
        let burnt_funds_head = self.put_store(&AccountState {
            address: BURNT_FUNDS_ACTOR_ADDR,
        });
        self.set_actor(
            BURNT_FUNDS_ACTOR_ADDR,
            actor(
                self.get_actor_code(Type::Account),
                burnt_funds_head,
                0,
                TokenAmount::zero(),
                None,
            ),
        );
    }

    pub fn mock_ethaccount_actor(&mut self, addr: Address, balance: TokenAmount, nonce: u64) {
        let mut id_addr = Address::new_id(0);
        self.mutate_state(INIT_ACTOR_ADDR, |st: &mut InitState| {
            let (addr_id, exist) = st.map_addresses_to_id(self.store, &addr, None).unwrap();
            assert!(
                !exist,
                "should never have existing actor when no f4 address is specified"
            );
            id_addr = Address::new_id(addr_id);
        });
        self.set_actor(
            id_addr,
            actor(
                self.get_actor_code(Type::EthAccount),
                EMPTY_ARR_CID,
                nonce,
                balance,
                Some(addr),
            ),
        );
    }

    pub fn mock_evm_actor(&mut self, addr: Address, balance: TokenAmount, nonce: u64) {
        let mut id_addr = Address::new_id(0);
        let robust_address = Address::new_actor(&addr.to_bytes());
        self.mutate_state(INIT_ACTOR_ADDR, |st: &mut InitState| {
            let (addr_id, exist) = st
                .map_addresses_to_id(self.store, &robust_address, Some(&addr))
                .unwrap();
            assert!(
                !exist,
                "should never have existing actor when no f4 address is specified"
            );
            id_addr = Address::new_id(addr_id);
        });
        self.set_actor(
            id_addr,
            actor(
                self.get_actor_code(Type::EVM),
                EMPTY_ARR_CID,
                nonce,
                balance,
                Some(addr),
            ),
        );
    }

    pub fn hash(&self, hasher: SupportedHashes, data: &[u8]) -> Vec<u8> {
        let hasher = Code::try_from(hasher as u64).unwrap();
        let (_, digest, written) = hasher.digest(data).into_inner();
        Vec::from(&digest[..written as usize])
    }

    pub fn mock_evm_actor_state(
        &mut self,
        addr: &Address,
        storage: HashMap<U256, U256>,
        bytecode: Option<Vec<u8>>,
    ) -> anyhow::Result<()> {
        if storage.len() == 0 && bytecode.is_none() {
            return Ok(());
        }
        let addr = self
            .normalize_address(addr)
            .expect("failed to normalize address");
        let head = self.get_actor(addr).unwrap().head;
        let (mut slots, bytecode_cid, bytecode_hash, nonce) =
            match self.store.get_cbor::<EvmState>(&head) {
                Ok(res) => match res {
                    Some(state) => {
                        let slots = StateKamt::load_with_config(
                            &state.contract_state,
                            self.store,
                            KAMT_CONFIG.clone(),
                        )
                        .context_code(ExitCode::USR_ILLEGAL_STATE, "state not in blockstore")
                        .unwrap();
                        (
                            slots,
                            Some(state.bytecode),
                            Some(state.bytecode_hash),
                            state.nonce,
                        )
                    }
                    None => {
                        let slots = StateKamt::new_with_config(self.store, KAMT_CONFIG.clone());
                        (slots, None, None, 1)
                    }
                },
                Err(_) => {
                    let slots = StateKamt::new_with_config(self.store, KAMT_CONFIG.clone());
                    (slots, None, None, 1)
                }
            };
        let mut unchanged = true;

        for (key, value) in storage {
            let changed = if value.is_zero() {
                slots.delete(&key).map(|v| v.is_some())
            } else {
                slots.set(key, value).map(|v| v != Some(value))
            }
            .map_err(|e| StatusCode::InternalError(e.to_string()))
            .unwrap();
            if changed {
                unchanged = false;
            }
        }

        let generate = |bytecode: Vec<u8>| -> (MultihashGeneric<64_usize>, Cid) {
            let code_hash = multihash::Multihash::wrap(
                SupportedHashes::Keccak256 as u64,
                &self.hash(SupportedHashes::Keccak256, &bytecode),
            )
            .context_code(
                ExitCode::USR_ILLEGAL_STATE,
                "failed to hash bytecode with keccak",
            )
            .unwrap();
            let bytecode_cid = self
                .store
                .put(Code::Blake2b256, &Block::new(IPLD_RAW, bytecode))
                .context_code(ExitCode::USR_ILLEGAL_STATE, "failed to write bytecode")
                .unwrap();
            (code_hash, bytecode_cid)
        };

        let (bytecode_hash, bytecode) = if let Some(bytecode_cid) = bytecode_cid {
            if let Some(bytecode) = bytecode {
                let old_bytecode = self
                    .store
                    .get(&bytecode_cid)
                    .context_code(ExitCode::USR_NOT_FOUND, "failed to read bytecode")
                    .unwrap()
                    .expect("bytecode not in state tree");
                if bytecode.eq(&old_bytecode) {
                    (bytecode_hash.unwrap(), bytecode_cid)
                } else {
                    unchanged = false;
                    generate(bytecode)
                }
            } else {
                (bytecode_hash.unwrap(), bytecode_cid)
            }
        } else {
            let bytecode = if let Some(bytecode) = bytecode {
                unchanged = false;
                bytecode
            } else {
                vec![0u8; 0]
            };
            generate(bytecode)
        };

        if unchanged {
            return Ok(());
        }
        let new_root = self
            .store
            .put_cbor(
                &EvmState {
                    bytecode,
                    bytecode_hash,
                    contract_state: slots
                        .flush()
                        .context_code(
                            ExitCode::USR_ILLEGAL_STATE,
                            "failed to flush contract state",
                        )
                        .unwrap(),
                    nonce,
                },
                Code::Blake2b256,
            )
            .context_code(
                ExitCode::USR_ILLEGAL_STATE,
                "failed to write contract state",
            )
            .unwrap();

        let mut a = self.get_actor(addr).unwrap();
        a.head = new_root;
        self.set_actor(addr, a);

        Ok(())
    }

    pub fn mock_actor_balance(
        &mut self,
        addr: &Address,
        balance: TokenAmount,
        nonce: Option<u64>,
    ) -> anyhow::Result<()> {
        let addr = self
            .normalize_address(addr)
            .expect("failed to normalize address");
        let mut a = self.get_actor(addr).unwrap();
        let mut unchanged = true;
        if !a.balance.eq(&balance) {
            a.balance = balance;
            unchanged = false;
        }
        if let Some(nonce) = nonce {
            if a.nonce != nonce {
                a.nonce = nonce;
                unchanged = false;
            }
        }
        if !unchanged {
            self.set_actor(addr, a);
        }
        Ok(())
    }

    pub fn get_actors(&self) -> Cid {
        let cid: &Cid = &self.actors.borrow();
        cid.clone()
    }

    pub fn put_store<S>(&self, obj: &S) -> Cid
    where
        S: serde::ser::Serialize,
    {
        self.store.put_cbor(obj, Code::Blake2b256).unwrap()
    }

    pub fn get_state<T: DeserializeOwned>(&self, addr: Address) -> Option<T> {
        let a_opt = self.get_actor(addr);
        if a_opt == None {
            return None;
        };
        let a = a_opt.unwrap();
        self.store.get_cbor::<T>(&a.head).unwrap()
    }

    pub fn set_actor(&mut self, actor_addr: Address, actor: Actor) -> () {
        let mut actors = Hamt::<&BS, Actor>::load_with_bit_width(
            &self.actors.borrow(),
            self.store,
            HAMT_BIT_WIDTH,
        )
        .unwrap();
        actors.set(actor_addr.to_bytes().into(), actor).unwrap();
        self.actors.replace(actors.flush().unwrap());
    }

    pub fn get_actor(&self, addr: Address) -> Option<Actor> {
        let actors = Hamt::<&BS, Actor>::load_with_bit_width(
            &self.actors.borrow(),
            self.store,
            HAMT_BIT_WIDTH,
        )
        .unwrap();
        actors.get(&addr.to_bytes()).unwrap().cloned()
    }

    pub fn normalize_address(&self, addr: &Address) -> Option<Address> {
        let st = self.get_state::<InitState>(INIT_ACTOR_ADDR).unwrap();
        st.resolve_address::<BS>(self.store, addr).unwrap()
    }

    pub fn mutate_state<S, F>(&mut self, addr: Address, f: F)
    where
        S: Serialize + DeserializeOwned,
        F: FnOnce(&mut S),
    {
        let mut a = self.get_actor(addr).unwrap();
        let mut st = self.store.get_cbor::<S>(&a.head).unwrap().unwrap();
        f(&mut st);
        a.head = self.store.put_cbor(&st, Code::Blake2b256).unwrap();
        self.set_actor(addr, a);
    }

    pub fn get_actor_code(&self, actor_type: Type) -> Cid {
        self.actor_codes.get(&actor_type).unwrap().clone()
    }

    pub fn print_evm_actors(&self, identifier: impl Display, actors: Cid) -> anyhow::Result<()> {
        log::info!("--- {} evm actors, actors:{} ---", identifier, actors);
        let actors = Hamt::<&BS, Actor>::load_with_bit_width(&actors, self.store, HAMT_BIT_WIDTH)?;
        actors.for_each(|_, v| {
            let head = v.head;
            let store = self.store.clone();
            match store.get_cbor::<EvmState>(&head) {
                Ok(res) => match res {
                    Some(state) => {
                        if v.predictable_address.is_some() {
                            let receiver_eth_addr =
                                address_to_eth(&v.predictable_address.unwrap())?;
                            let addr =
                                Address::new_delegated(EAM_ACTOR_ID, &receiver_eth_addr.0).unwrap();
                            let addr = self
                                .normalize_address(&addr)
                                .expect("failed to normalize address");
                            log::info!(
                                "--- actor_id:{} actor_address:{} eth_addr:{} ---",
                                addr,
                                &v.predictable_address.unwrap(),
                                hex::encode(receiver_eth_addr.0)
                            );
                        }
                        log::info!("actor: {:?}", v);
                        log::info!("state: {:?}", &state);
                        let bytecode = store
                            .get(&state.bytecode)
                            .context_code(ExitCode::USR_NOT_FOUND, "failed to read bytecode")?
                            .expect("bytecode not in state tree");
                        log::debug!("bytecode: {:?}", hex::encode(bytecode));
                        let slots = StateKamt::load_with_config(
                            &state.contract_state,
                            store,
                            KAMT_CONFIG.clone(),
                        )
                        .context_code(ExitCode::USR_ILLEGAL_STATE, "state not in blockstore")?;
                        if !slots.is_empty() {
                            log::info!("slots:");
                            slots.for_each(|k, v| {
                                log::info!(
                                    "0x{}: 0x{}",
                                    hex::encode(u256_to_bytes(k)),
                                    hex::encode(u256_to_bytes(v))
                                );
                                Ok(())
                            })?;
                        }
                    }
                    None => {}
                },
                Err(_) => {}
            }
            Ok(())
        })?;
        log::info!("\n");
        Ok(())
    }
}

pub fn address_to_eth(addr: &Address) -> anyhow::Result<EthAddress> {
    let delegated_addr = match addr.payload() {
        Payload::Delegated(delegated) if delegated.namespace() == EAM_ACTOR_ID => {
            // sanity check
            assert_eq!(delegated.subaddress().len(), 20);
            Ok(*delegated)
        }
        _ => Err(ActorError::assertion_failed(format!(
            "EVM actor with delegated address {} created not namespaced to the EAM {}",
            addr, EAM_ACTOR_ID,
        ))),
    }?;
    let receiver_eth_addr = {
        let subaddr: [u8; 20] = delegated_addr.subaddress().try_into().map_err(|_| {
            ActorError::assertion_failed(format!(
                "expected 20 byte EVM address, found {} bytes",
                delegated_addr.subaddress().len()
            ))
        })?;
        EthAddress(subaddr)
    };
    Ok(receiver_eth_addr)
}
