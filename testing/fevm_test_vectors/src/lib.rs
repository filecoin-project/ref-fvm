use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Once};

use async_std::channel::bounded;
use async_std::sync::RwLock;
use bytes::Buf;
use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
use ethers::abi::AbiEncode;
use fil_actor_eam::EthAddress;
use fil_actor_evm::interpreter::system::StateKamt;
use fil_actor_evm::interpreter::U256;
use fil_actors_runtime::runtime::builtins::Type;
use fil_actors_runtime::runtime::EMPTY_ARR_CID;
use fil_actors_runtime::{AsActorError, BURNT_FUNDS_ACTOR_ID, EAM_ACTOR_ID, REWARD_ACTOR_ID};
use flate2::bufread::GzEncoder;
use flate2::Compression;
use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
use fvm_ipld_car::CarHeader;
use fvm_ipld_encoding::{BytesDe, Cbor, CborStore, RawBytes, DAG_CBOR};
use fvm_ipld_hamt::Hamt;
use fvm_shared::address::Address;
use fvm_shared_local::address::Address as LocalAddress;
use fvm_shared::bigint::{BigInt, Integer};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::crypto::hash::SupportedHashes;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::message::Message;
use fvm_shared::receipt::Receipt;
use fvm_shared::state::StateRoot;
use fvm_shared::version::NetworkVersion;
use fvm_shared::{MethodNum, HAMT_BIT_WIDTH, IDENTITY_HASH, METHOD_SEND};
use util::get_code_cid_map;

use crate::evm_state::State as EvmState;
use crate::extractor::types::EthTransactionTestVector;
use crate::mock::{address_to_eth, Actor, Mock, KAMT_CONFIG};
use crate::tracing_blockstore::TracingBlockStore;
use crate::types::{ContractParams, CreateParams};
use crate::util::{compute_address_create, hex_to_u256, u256_to_bytes};
use conformance::vector::{MessageVector, GenerationData, MetaData, RandomnessMatch, RandomnessRule, TipsetCid, Variant, PreConditions, StateTreeVector, ApplyMessage, PostConditions, RandomnessKind};

pub mod evm_state;
pub mod extractor;
pub mod mock;
pub mod tracing_blockstore;
pub mod types;
pub mod util;

const LOG_INIT: Once = Once::new();

#[inline(always)]
pub fn init_log() {
    LOG_INIT.call_once(|| {
        fil_logger::init();
    });
}

pub async fn export_test_vector_file(
    input: EthTransactionTestVector,
    path: PathBuf,
) -> anyhow::Result<()> {
    let _debug = serde_json::to_string(&input).unwrap();
    let actor_codes = get_code_cid_map()?;
    let store = TracingBlockStore::new(MemoryBlockstore::new());

    let (pre_actors, post_actors, contract_addrs) =
        load_evm_contract_input(&store, actor_codes, &input)?;
    let pre_state_root = store.put_cbor(
        &StateRoot {
            version: fvm_shared::state::StateTreeVersion::V5,
            actors: pre_actors,
            info: EMPTY_ARR_CID,
        },
        Code::Blake2b256,
    )?;
    let post_state_root = store.put_cbor(
        &StateRoot {
            version: fvm_shared::state::StateTreeVersion::V5,
            actors: post_actors,
            info: EMPTY_ARR_CID,
        },
        Code::Blake2b256,
    )?;

    //car_bytes
    let car_header = CarHeader::new(vec![pre_state_root, post_state_root], 1);
    let (tx, mut rx) = bounded(100);
    let buffer: Arc<RwLock<Vec<u8>>> = Default::default();
    let buffer_cloned = buffer.clone();
    let write_task = async_std::task::spawn(async move {
        car_header
            .write_stream_async(&mut *buffer_cloned.write().await, &mut rx)
            .await
            .unwrap()
    });
    for cid in (&store).traced.borrow().iter() {
        tx.send((cid.clone(), store.base.get(cid).unwrap().unwrap()))
            .await
            .unwrap();
    }
    drop(tx);
    write_task.await;
    let car_bytes = buffer.read().await.clone();

    //gzip car_bytes
    let mut gz_car_bytes: Vec<u8> = Default::default();
    let mut gz_encoder = GzEncoder::new(car_bytes.reader(), Compression::new(9));
    gz_encoder.read_to_end(&mut gz_car_bytes).unwrap();

    //message
    let message = to_message(&input);

    //receipt
    let receipt = fvm_shared_local::receipt::Receipt {
        exit_code: fvm_shared_local::error::ExitCode::OK,
        return_data: fvm_ipld_encoding_local::RawBytes::serialize(BytesDe(input.return_value.to_vec()))?,
        gas_used: 0,
        events_root: None,
    };
    log::info!("receipt: {:?}", receipt);

    // tipset_cids
    let mut tipset_cids = Vec::new();
    for (block_number, block_hash) in input.block_hashes {
        tipset_cids.push(TipsetCid {
            epoch: block_number as ChainEpoch,
            cid: Cid::new_v1(
                DAG_CBOR,
                multihash::Multihash::wrap(IDENTITY_HASH, &block_hash.0).unwrap(),
            ),
        });
    }

    const ENTROPY: &[u8] = b"prevrandao";
    let ret = {
        let mut bytes = [0; 32];
        input.random.to_big_endian(&mut bytes);
        bytes.to_vec()
    };
    let randomness = vec![RandomnessMatch {
        on: RandomnessRule {
            kind: RandomnessKind::Beacon,
            dst: 10, //fil_actors_runtime::runtime::randomness::DomainSeparationTag::EvmPrevRandao as i64,
            epoch: input.block_number as ChainEpoch,
            entropy: Vec::from(ENTROPY),
        },
        ret,
    }];
    let variants = vec![Variant {
        id: String::from("test_evm"),
        epoch: input.block_number as ChainEpoch,
        timestamp: Some(input.timestamp.as_u64()),
        nv: NetworkVersion::V18 as u32,
    }];
    let test_vector = MessageVector {
        chain_id: Some(input.chain_id.as_u64()),
        selector: None,
        meta: Some(MetaData {
            id: input.hash.encode_hex(),
            version: String::from(""),
            description: String::from(""),
            comment: String::from(""),
            gen: vec![GenerationData {
                source: env!("CARGO_PKG_REPOSITORY").to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            }],
            _debug
        }),
        car: gz_car_bytes,
        preconditions: PreConditions {
            state_tree: StateTreeVector {
                root_cid: pre_state_root,
            },
            basefee: None,
            circ_supply: None,
            variants,
        },
        apply_messages: vec![ApplyMessage {
            bytes: message.marshal_cbor()?,
            epoch_offset: None,
        }],
        postconditions: PostConditions {
            state_tree: StateTreeVector {
                root_cid: post_state_root,
            },
            receipts: vec![receipt],
            receipts_roots: vec![]
        },
        skip_compare_gas_used: true,
        skip_compare_addresses: Some(vec![LocalAddress::from_bytes(&message.from.to_bytes()).unwrap()]),
        skip_compare_actor_ids: Some(vec![REWARD_ACTOR_ID, BURNT_FUNDS_ACTOR_ID]),
        additional_compare_addresses: Some(
            contract_addrs
                .into_iter()
                .filter(|contract_addr| contract_addr != &message.to)
                .collect::<Vec<Address>>()
                .into_iter()
                .map(|e| LocalAddress::from_bytes(&e.to_bytes()).unwrap())
                .collect(),
        ),
        tipset_cids: Some(tipset_cids),
        randomness,
    };

    let output = File::create(&path)?;
    serde_json::to_writer_pretty(output, &test_vector)?;
    Ok(())
}

pub fn load_evm_contract_input<BS>(
    store: &BS,
    actor_codes: BTreeMap<Type, Cid>,
    input: &EthTransactionTestVector,
) -> anyhow::Result<(Cid, Cid, Vec<Address>)>
where
    BS: Blockstore,
{
    let mut contract_addrs = Vec::new();

    let mut mock = Mock::new(store, actor_codes);
    mock.mock_builtin_actor();

    let from = Address::new_delegated(EAM_ACTOR_ID, &input.from.0).unwrap();
    mock.mock_ethaccount_actor(from, TokenAmount::from_whole(100000000), input.nonce);

    // preconditions
    let create_contract_eth_addr = if input.create_contract() {
        Some(compute_address_create(
            &EthAddress(input.from.0),
            input.nonce,
        ))
    } else {
        None
    };
    for (k, state) in &input.prestate {
        let eth_addr = EthAddress(k.0);
        let to = Address::new_delegated(EAM_ACTOR_ID, &eth_addr.0).unwrap();
        let balance = TokenAmount::from_atto(state.get_balance());
        if eth_addr.eq(&EthAddress(input.from.0)) {
            continue;
        }

        contract_addrs.push(to.clone());

        if let Some(create_contract_eth_addr) = create_contract_eth_addr {
            if eth_addr.eq(&create_contract_eth_addr) {
                continue;
            }
        }
        mock.mock_evm_actor(to, balance, state.nonce);
        let mut storage = HashMap::<U256, U256>::new();
        for (k, v) in &state.storage {
            let key = hex_to_u256(&hex::encode(k.0));
            let value = hex_to_u256(&hex::encode(v.0));
            storage.insert(key, value);
        }
        mock.mock_evm_actor_state(&to, storage, Some(state.code.to_vec()))?;
    }
    let pre_actors = mock.get_actors();
    mock.print_evm_actors("pre", pre_actors)?;

    // postconditions
    for (k, state) in &input.poststate {
        let eth_addr = EthAddress(k.0);
        let to = Address::new_delegated(EAM_ACTOR_ID, &eth_addr.0).unwrap();
        let balance = TokenAmount::from_atto(state.get_balance());
        if eth_addr.eq(&EthAddress(input.from.0)) {
            continue;
        }
        if let Some(create_contract_eth_addr) = create_contract_eth_addr {
            if eth_addr.eq(&create_contract_eth_addr) {
                mock.mock_evm_actor(to, balance.clone(), state.nonce);
            }
        }
        let mut storage = HashMap::<U256, U256>::new();
        for (k, v) in &state.storage {
            let key = hex_to_u256(&hex::encode(k.0));
            let value = hex_to_u256(&hex::encode(v.0));
            storage.insert(key, value);
        }
        mock.mock_evm_actor_state(&to, storage, Some(state.code.to_vec()))?;
        mock.mock_actor_balance(&to, balance, Some(state.nonce))?;
    }
    let post_actors = mock.get_actors();
    mock.print_evm_actors("post", post_actors)?;

    return Ok((pre_actors, post_actors, contract_addrs));
}

pub fn to_message(context: &EthTransactionTestVector) -> Message {
    let from = Address::new_delegated(EAM_ACTOR_ID, &context.from.0).unwrap();
    let to: Address;
    let method_num: MethodNum;
    let mut params = RawBytes::from(vec![0u8; 0]);
    if context.create_contract() {
        to = Address::new_id(10);
        method_num = fil_actor_eam::Method::Create as u64;
        let params2 = CreateParams {
            initcode: context.input.to_vec(),
            nonce: context.nonce,
        };
        params = RawBytes::serialize(params2).unwrap();
    } else {
        to = Address::new_delegated(EAM_ACTOR_ID, &context.to.0).unwrap();
        if context.input.len() > 0 {
            params = RawBytes::serialize(ContractParams(context.input.to_vec())).unwrap();
            method_num = fil_actor_evm::Method::InvokeContract as u64
        } else {
            method_num = METHOD_SEND;
        }
    }
    Message {
        version: 0,
        from,
        to,
        sequence: context.nonce,
        value: TokenAmount::from_atto(context.get_value()),
        method_num,
        params,
        gas_limit: (context.gas.as_u64() * 1000000) as i64,
        gas_fee_cap: TokenAmount::from_atto(context.get_max_fee_per_gas()),
        gas_premium: TokenAmount::from_atto(context.get_max_priority_fee_per_gas()),
    }
}

pub fn get_evm_actors_slots<BS: Blockstore>(
    identifier: impl Display,
    state_root: Cid,
    store: &BS,
) -> anyhow::Result<HashMap<String, HashMap<U256, U256>>> {
    println!(
        "--- {} evm actors, state_root:{} ---",
        identifier, state_root
    );
    let mut states = HashMap::new();
    let actors = Hamt::<&BS, Actor>::load_with_bit_width(&state_root, store, HAMT_BIT_WIDTH)?;
    actors.for_each(|_, v| {
        let state_root = v.head;
        let store = store.clone();
        match store.get_cbor::<EvmState>(&state_root) {
            Ok(res) => match res {
                Some(state) => {
                    if v.predictable_address.is_some() {
                        let receiver_eth_addr = address_to_eth(&v.predictable_address.unwrap())?;
                        println!(
                            "--- actor_address:{} eth_addr:{} ---",
                            &v.predictable_address.unwrap(),
                            hex::encode(receiver_eth_addr.0)
                        );
                        println!("actor: {:?}", v);
                        println!("state: {:?}", &state);
                        let mut storage = HashMap::new();
                        let slots = StateKamt::load_with_config(
                            &state.contract_state,
                            store,
                            KAMT_CONFIG.clone(),
                        )
                        .context_code(ExitCode::USR_ILLEGAL_STATE, "state not in blockstore")?;
                        if !slots.is_empty() {
                            println!("slots:");
                            slots.for_each(|k, v| {
                                println!(
                                    "0x{}: 0x{}",
                                    hex::encode(u256_to_bytes(k)),
                                    hex::encode(u256_to_bytes(v))
                                );
                                storage.insert(k.clone(), v.clone());
                                Ok(())
                            })?;
                            states.insert(hex::encode(receiver_eth_addr.0), storage);
                        }
                    }
                }
                None => {}
            },
            Err(_) => {}
        }
        Ok(())
    })?;
    Ok(states)
}
