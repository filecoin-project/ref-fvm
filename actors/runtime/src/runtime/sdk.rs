use anyhow::Error;
use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};

use crate::runtime::actor_blockstore::ActorBlockstore;
use crate::runtime::{ConsensusFault, MessageInfo, Syscalls};
use crate::Runtime;
use crate::{actor_error, ActorError};
use blockstore::Blockstore;
use fvm_sdk as fvm;
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{to_vec, Cbor, CborStore, RawBytes, DAG_CBOR};
use fvm_shared::randomness::Randomness;
use fvm_shared::sector::{AggregateSealVerifyProofAndInfos, SealVerifyInfo, WindowPoStVerifyInfo};
use fvm_shared::version::NetworkVersion;
use fvm_shared::MethodNum;

lazy_static! {
    /// Cid of the empty array Cbor bytes (`EMPTY_ARR_BYTES`).
    pub static ref EMPTY_ARR_CID: Cid = {
        let empty = to_vec::<[(); 0]>(&[]).unwrap();
        Cid::new_v1(DAG_CBOR, Code::Blake2b256.digest(&empty))
    };
}

pub struct FvmRuntime<B> {
    blockstore: B,
    in_transaction: bool,
}

struct FvmMessage;

impl MessageInfo for FvmMessage {
    fn caller(&self) -> Address {
        Address::new_id(fvm::message::caller().unwrap())
    }

    fn receiver(&self) -> Address {
        Address::new_id(fvm::message::receiver().unwrap())
    }

    fn value_received(&self) -> TokenAmount {
        fvm::message::value_received().unwrap()
    }
}

impl<B> Runtime<B> for FvmRuntime<B>
where
    B: Blockstore,
{
    fn network_version(&self) -> NetworkVersion {
        fvm::network::version().unwrap()
    }

    fn message(&self) -> &dyn MessageInfo {
        &FvmMessage
    }

    fn curr_epoch(&self) -> ChainEpoch {
        fvm::network::curr_epoch().unwrap()
    }

    fn validate_immediate_caller_accept_any(&mut self) -> Result<(), ActorError> {
        Ok(fvm::validation::validate_immediate_caller_accept_any()?)
    }

    fn validate_immediate_caller_is<'a, I>(&mut self, addresses: I) -> Result<(), ActorError>
    where
        I: IntoIterator<Item = &'a Address>,
    {
        let addrs = addresses.into_iter().map(|e| *e).collect::<Vec<Address>>();
        Ok(fvm::validation::validate_immediate_caller_addr_one_of(
            addrs.as_slice(),
        )?)
    }

    fn validate_immediate_caller_type<'a, I>(&mut self, types: I) -> Result<(), ActorError>
    where
        I: IntoIterator<Item = &'a Cid>,
    {
        let cids = types.into_iter().map(|e| *e).collect::<Vec<Cid>>();
        Ok(fvm::validation::validate_immediate_caller_type_one_of(
            cids.as_slice(),
        )?)
    }

    fn current_balance(&self) -> Result<TokenAmount, ActorError> {
        Ok(fvm::sself::current_balance()?)
    }

    fn resolve_address(&self, address: &Address) -> Result<Option<Address>, ActorError> {
        Ok(fvm::actor::resolve_address(*address)?.map(Address::new_id))
    }

    fn get_actor_code_cid(&self, addr: &Address) -> Result<Option<Cid>, ActorError> {
        Ok(fvm::actor::get_actor_code_cid(*addr)?)
    }

    fn get_randomness_from_tickets(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<Randomness, ActorError> {
        Ok(fvm::rand::get_chain_randomness(
            personalization,
            rand_epoch,
            entropy,
        )?)
    }

    fn get_randomness_from_beacon(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<Randomness, ActorError> {
        Ok(fvm::rand::get_beacon_randomness(
            personalization,
            rand_epoch,
            entropy,
        )?)
    }

    fn create<C: Cbor>(&mut self, obj: &C) -> Result<(), ActorError> {
        let root = fvm::sself::get_root()?;
        if root != *EMPTY_ARR_CID {
            return Err(actor_error!(fatal(
                "failed to create state; expected empty array CID, got: {}",
                root
            )));
        }
        let new_root = ActorBlockstore.put_cbor(obj, Code::Blake2b256)
            .map_err(|e| actor_error!(SysErrIllegalArgument; "failed to write actor state during creation: {}", e.to_string()))?;
        fvm::sself::set_root(&new_root)?;
        Ok(())
    }

    fn state<C: Cbor>(&self) -> Result<C, ActorError> {
        let root = fvm::sself::get_root()?;
        Ok(ActorBlockstore
            .get_cbor(&root)
            .map_err(
                |_| actor_error!(SysErrIllegalArgument; "failed to get actor for Readonly state"),
            )?
            .ok_or(actor_error!(fatal(
                "State does not exist for actor state root"
            )))?)
    }

    fn transaction<C, RT, F>(&mut self, f: F) -> Result<RT, ActorError>
    where
        C: Cbor,
        F: FnOnce(&mut C, &mut Self) -> Result<RT, ActorError>,
    {
        let state_cid = fvm::sself::get_root().map_err(
            |_| actor_error!(SysErrIllegalArgument; "failed to get actor root state CID"),
        )?;
        let mut state = ActorBlockstore
            .get_cbor::<C>(&state_cid)
            .map_err(|_| actor_error!(SysErrIllegalArgument; "failed to get actor state"))?
            .ok_or_else(|| {
                actor_error!(fatal(
                    "State does not exist for actor state cid: {}",
                    state_cid
                ))
            })?;

        self.in_transaction = true;
        let result = f(&mut state, self);
        self.in_transaction = false;

        let ret = result?;
        let new_root = ActorBlockstore.put_cbor(&state, Code::Blake2b256)
            .map_err(|e| actor_error!(SysErrIllegalArgument; "failed to write actor state in transaction: {}", e.to_string()))?;
        fvm::sself::set_root(&new_root)?;
        Ok(ret)
    }

    fn store(&self) -> &B {
        &self.blockstore
    }

    fn send(
        &mut self,
        to: Address,
        method: MethodNum,
        params: RawBytes,
        value: TokenAmount,
    ) -> Result<RawBytes, ActorError> {
        if self.in_transaction {
            return Err(actor_error!(SysErrIllegalActor; "runtime.send() is not allowed"));
        }
        Ok(fvm::send::send(&to, method, params, value)?.return_data)
    }

    fn new_actor_address(&mut self) -> Result<Address, ActorError> {
        Ok(fvm::actor::new_actor_address()?)
    }

    fn create_actor(&mut self, code_id: Cid, address: &Address) -> Result<(), ActorError> {
        Ok(fvm::actor::create_actor(*address, code_id)?)
    }

    fn delete_actor(&mut self, beneficiary: &Address) -> Result<(), ActorError> {
        Ok(fvm::sself::self_destruct(*beneficiary)?)
    }

    fn total_fil_circ_supply(&self) -> Result<TokenAmount, ActorError> {
        // TODO: Why hasn't Aayush done this yet, very disappointing
        todo!()
    }

    fn charge_gas(&mut self, name: &'static str, compute: i64) -> Result<(), ActorError> {
        Ok(fvm::gas::charge(name, compute as u64)?)
    }

    fn base_fee(&self) -> TokenAmount {
        fvm::network::base_fee().unwrap()
    }
}

impl<B> Syscalls for FvmRuntime<B>
where
    B: Blockstore,
{
    fn verify_signature(
        &self,
        signature: &Signature,
        signer: &Address,
        plaintext: &[u8],
    ) -> Result<(), Error> {
        match fvm::crypto::verify_signature(signature, signer, plaintext) {
            Ok(true) => Ok(()),
            Ok(false) | Err(_) => Err(Error::msg("invalid signature")),
        }
    }

    fn verify_seal(&self, vi: &SealVerifyInfo) -> Result<(), Error> {
        match fvm::crypto::verify_seal(vi) {
            Ok(true) => Ok(()),
            Ok(false) | Err(_) => Err(Error::msg("invalid seal")),
        }
    }

    fn verify_post(&self, verify_info: &WindowPoStVerifyInfo) -> Result<(), Error> {
        match fvm::crypto::verify_post(verify_info) {
            Ok(true) => Ok(()),
            Ok(false) | Err(_) => Err(Error::msg("invalid post")),
        }
    }

    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> Result<Option<ConsensusFault>, Error> {
        fvm::crypto::verify_consensus_fault(h1, h2, extra).map_err(|_| Error::msg("no fault"))
    }

    fn verify_aggregate_seals(
        &self,
        aggregate: &AggregateSealVerifyProofAndInfos,
    ) -> Result<(), Error> {
        match fvm::crypto::verify_aggregate_seals(aggregate) {
            Ok(true) => Ok(()),
            Ok(false) | Err(_) => Err(Error::msg("invalid aggregate")),
        }
    }
}
