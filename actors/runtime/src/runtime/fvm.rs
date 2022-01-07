use anyhow::{anyhow, Error};
use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};
use std::convert::TryInto;

use crate::runtime::actor_blockstore::ActorBlockstore;
use crate::runtime::{ActorCode, ConsensusFault, MessageInfo, Syscalls};
use crate::Runtime;
use crate::{actor_error, ActorError};
use fvm_sdk as fvm;
use fvm_shared::blockstore::{Blockstore, CborStore};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{to_vec, Cbor, RawBytes, DAG_CBOR};
use fvm_shared::piece::PieceInfo;
use fvm_shared::randomness::Randomness;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, SealVerifyInfo, WindowPoStVerifyInfo,
};
use fvm_shared::version::NetworkVersion;
use fvm_shared::MethodNum;
use fvm_shared::{address::Address, ActorID};

lazy_static! {
    /// Cid of the empty array Cbor bytes (`EMPTY_ARR_BYTES`).
    pub static ref EMPTY_ARR_CID: Cid = {
        let empty = to_vec::<[(); 0]>(&[]).unwrap();
        Cid::new_v1(DAG_CBOR, Code::Blake2b256.digest(&empty))
    };
}

/// A runtime that bridges to the FVM environment through the FVM SDK.
pub struct FvmRuntime<B = ActorBlockstore> {
    blockstore: B,
    /// Indicates whether we are in a state transaction. During such, sending
    /// messages is prohibited.
    in_transaction: bool,
}

impl Default for FvmRuntime {
    fn default() -> Self {
        FvmRuntime {
            blockstore: ActorBlockstore,
            in_transaction: false,
        }
    }
}

/// A stub MessageInfo implementation performing FVM syscalls to obtain its fields.
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
        let root = fvm::sself::root()?;
        if root != *EMPTY_ARR_CID {
            return Err(
                actor_error!(ErrIllegalState; "failed to create state; expected empty array CID, got: {}", root),
            );
        }
        let new_root = ActorBlockstore.put_cbor(obj, Code::Blake2b256)
            .map_err(|e| actor_error!(ErrIllegalArgument; "failed to write actor state during creation: {}", e.to_string()))?;
        fvm::sself::set_root(&new_root)?;
        Ok(())
    }

    fn state<C: Cbor>(&self) -> Result<C, ActorError> {
        let root = fvm::sself::root()?;
        Ok(ActorBlockstore
            .get_cbor(&root)
            .map_err(
                |_| actor_error!(ErrIllegalArgument; "failed to get actor for Readonly state"),
            )?
            .expect("State does not exist for actor state root"))
    }

    fn transaction<C, RT, F>(&mut self, f: F) -> Result<RT, ActorError>
    where
        C: Cbor,
        F: FnOnce(&mut C, &mut Self) -> Result<RT, ActorError>,
    {
        let state_cid = fvm::sself::root()
            .map_err(|_| actor_error!(ErrIllegalArgument; "failed to get actor root state CID"))?;

        let mut state = ActorBlockstore
            .get_cbor::<C>(&state_cid)
            .map_err(|_| actor_error!(ErrIllegalArgument; "failed to get actor state"))?
            .expect("State does not exist for actor state root");

        self.in_transaction = true;
        let result = f(&mut state, self);
        self.in_transaction = false;

        let ret = result?;
        let new_root = ActorBlockstore.put_cbor(&state, Code::Blake2b256)
            .map_err(|e| actor_error!(ErrIllegalArgument; "failed to write actor state in transaction: {}", e.to_string()))?;
        fvm::sself::set_root(&new_root)?;
        Ok(ret)
    }

    fn store(&self) -> &B {
        &self.blockstore
    }

    fn send(
        &self,
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

    fn create_actor(&mut self, code_id: Cid, actor_id: ActorID) -> Result<(), ActorError> {
        Ok(fvm::actor::create_actor(actor_id, code_id)?)
    }

    fn delete_actor(&mut self, beneficiary: &Address) -> Result<(), ActorError> {
        Ok(fvm::sself::self_destruct(*beneficiary)?)
    }

    fn total_fil_circ_supply(&self) -> Result<TokenAmount, ActorError> {
        Ok(fvm::network::total_fil_circ_supply()?)
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

    fn hash_blake2b(&self, data: &[u8]) -> Result<[u8; 32], Error> {
        fvm::crypto::hash_blake2b(data)
            .map_err(|e| anyhow!("failed to compute blake2b hash; exit code: {}", e))?
            .try_into()
            .map_err(|v: Vec<u8>| anyhow!("unexpected hash length; expected 32, got: {}", v.len()))
    }

    fn compute_unsealed_sector_cid(
        &self,
        proof_type: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Result<Cid, Error> {
        // The only actor that invokes this (market actor) is generating the
        // exit code ErrIllegalArgument. We should probably move that here, or to the syscall itself.
        fvm::crypto::compute_unsealed_sector_cid(proof_type, pieces)
            .map_err(|e| anyhow!("failed to compute unsealed sector CID; exit code: {}", e))
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

/// A convenience function that built-in actors can delegate their execution to.
///
/// The trampoline takes care of boilerplate:
///
/// 1.  Obtains the parameter data from the FVM by fetching the parameters block.
/// 2.  Obtains the method number for the invocation.
/// 3.  Creates an FVM runtime shim.
/// 4.  Invokes the target method.
/// 5a. In case of error, aborts the execution with the emitted exit code, or
/// 5b. In case of success, stores the return data as a block and returns the latter.
pub fn trampoline<C: ActorCode>(params: u32) -> u32 {
    let method = fvm::message::method_number().expect("no method number");
    let params = if params > 0 {
        let params = fvm::message::params_raw(params)
            .expect("params block invalid")
            .1;
        RawBytes::new(params)
    } else {
        RawBytes::default()
    };

    fvm::debug::log(format!("[trampoline] input params: {:?}", params.bytes()));

    // Construct a new runtime.
    let mut rt = FvmRuntime::default();
    // Invoke the method, aborting if the actor returns an errored exit code, or
    // handling the return data appropriately otherwise.
    match C::invoke_method(&mut rt, method, &params) {
        Err(err) => fvm::vm::abort(err.exit_code() as u32, Some(err.msg())),
        Ok(ret) if ret.len() == 0 => 0,
        Ok(ret) => fvm::ipld::put_block(DAG_CBOR, ret.bytes()).expect("failed to write result"),
    }
}
