use anyhow::Error;
use cid::{multihash::Code, Cid};
use fvm::ipld;
use fvm_shared::error::{ActorError, ExitCode};
use std::convert::TryFrom;
use std::ops::Add;

use crate::runtime::{ConsensusFault, MessageInfo, Syscalls};
use crate::Runtime;
use blockstore::{Block, Blockstore};
use fvm_sdk as fvm;
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{Cbor, RawBytes};
use fvm_shared::message::Message;
use fvm_shared::randomness::Randomness;
use fvm_shared::sector::{AggregateSealVerifyProofAndInfos, SealVerifyInfo, WindowPoStVerifyInfo};
use fvm_shared::version::NetworkVersion;
use fvm_shared::MethodNum;

/// A blockstore suitable for use within actors.
pub struct ActorBlockstore;

pub struct SdkRuntime;

struct FvmMessage;

impl MessageInfo for FvmMessage {
    fn caller(&self) -> &Address {
        &Address::new_id(fvm::message::caller())
    }

    fn receiver(&self) -> &Address {
        &Address::new_id(fvm::message::receiver())
    }

    fn value_received(&self) -> &TokenAmount {
        &fvm::message::value_received()
    }
}

impl<B> Runtime<B> for SdkRuntime
where
    B: Blockstore,
{
    fn network_version(&self) -> NetworkVersion {
        fvm::network::version()
    }

    fn message(&self) -> &dyn MessageInfo {
        &FvmMessage
    }

    fn curr_epoch(&self) -> ChainEpoch {
        fvm::network::curr_epoch()
    }

    fn validate_immediate_caller_accept_any(&mut self) -> Result<(), ActorError> {
        // TODO rethrow error
        Ok(fvm::validation::validate_immediate_caller_accept_any())
    }

    fn validate_immediate_caller_is<'a, I>(&mut self, addresses: I) -> Result<(), ActorError>
    where
        I: IntoIterator<Item = &'a Address>,
    {
        // TODO rethrow error
        Ok(fvm::validation::validate_immediate_caller_addr_one_of(
            addresses.into_iter().collect(),
        ))
    }

    fn validate_immediate_caller_type<'a, I>(&mut self, types: I) -> Result<(), ActorError>
    where
        I: IntoIterator<Item = &'a Cid>,
    {
        // TODO rethrow error
        Ok(fvm::validation::validate_immediate_caller_type_one_of(
            types.into_iter().collect(),
        ))
    }

    fn current_balance(&self) -> Result<TokenAmount, ActorError> {
        // TODO rethrow error
        Ok(fvm::sself::current_balance())
    }

    fn resolve_address(&self, address: &Address) -> Result<Option<Address>, ActorError> {
        // TODO rethrow error
        Ok(fvm::actor::resolve_address(*address).map(Address::new_id))
    }

    fn get_actor_code_cid(&self, addr: &Address) -> Result<Option<Cid>, ActorError> {
        // TODO rethrow error
        Ok(fvm::actor::get_actor_code_cid(*addr))
    }

    fn get_randomness_from_tickets(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<Randomness, ActorError> {
        // TODO rethrow error
        Ok(fvm::rand::get_chain_randomness(
            personalization,
            rand_epoch,
            entropy,
        ))
    }

    fn get_randomness_from_beacon(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<Randomness, ActorError> {
        // TODO rethrow error
        Ok(fvm::rand::get_beacon_randomness(
            personalization,
            rand_epoch,
            entropy,
        ))
    }

    fn create<C: Cbor>(&mut self, obj: &C) -> Result<(), ActorError> {
        todo!()
    }

    fn state<C: Cbor>(&self) -> Result<C, ActorError> {
        todo!()
    }

    fn transaction<C, RT, F>(&mut self, f: F) -> Result<RT, ActorError>
    where
        C: Cbor,
        F: FnOnce(&mut C, &mut Self) -> Result<RT, ActorError>,
    {
        todo!()
    }

    fn store(&self) -> &B {
        &fvm::blockstore::Blockstore
    }

    fn send(
        &mut self,
        to: Address,
        method: MethodNum,
        params: RawBytes,
        value: TokenAmount,
    ) -> Result<RawBytes, ActorError> {
        // TODO: Aaaaahh, what about the other fields aaaaahhh
        Ok(fvm::send::send(Message {
            version: 0,
            from: *self.message().caller(),
            to,
            sequence: 0,
            value,
            method_num: method,
            params,
            gas_limit: 0,
            gas_fee_cap: Default::default(),
            gas_premium: Default::default(),
        })
        .return_data)
    }

    fn new_actor_address(&mut self) -> Result<Address, ActorError> {
        todo!()
    }

    fn create_actor(&mut self, code_id: Cid, address: &Address) -> Result<(), ActorError> {
        Ok(fvm::actor::create_actor(*address, code_id))
    }

    fn delete_actor(&mut self, beneficiary: &Address) -> Result<(), ActorError> {
        Ok(fvm::sself::self_destruct(*beneficiary))
    }

    fn total_fil_circ_supply(&self) -> Result<TokenAmount, ActorError> {
        todo!()
    }

    fn charge_gas(&mut self, name: &'static str, compute: i64) -> Result<(), ActorError> {
        Ok(fvm::gas::charge(name, compute as u64))
    }

    fn base_fee(&self) -> &TokenAmount {
        &fvm::network::base_fee()
    }
}

impl Syscalls for SdkRuntime {
    fn verify_signature(
        &self,
        signature: &Signature,
        signer: &Address,
        plaintext: &[u8],
    ) -> Result<(), Error> {
        fvm::crypto::verify_signature(signature, signer, plaintext)
            .then(|| ())
            .ok_or(Error::new("invalid signature"))
    }

    fn verify_seal(&self, vi: &SealVerifyInfo) -> Result<(), Error> {
        fvm::crypto::verify_seal(vi)
            .then(|| ())
            .ok_or(Error::new("invalid seal"))
    }

    fn verify_post(&self, verify_info: &WindowPoStVerifyInfo) -> Result<(), Error> {
        fvm::crypto::verify_post(verify_info)
            .then(|| ())
            .ok_or(Error::new("invalid post"))
    }

    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> Result<Option<ConsensusFault>, Error> {
        fvm::crypto::verify_consensus_fault(h1, h2, extra)
            .then(|| ())
            .ok_or(Error::new("no fault"))
    }

    fn verify_aggregate_seals(
        &self,
        aggregate: &AggregateSealVerifyProofAndInfos,
    ) -> Result<(), Error> {
        fvm::crypto::verify_aggregate_seals(aggregate)
            .then(|| ())
            .ok_or(Error::new("invalid aggregate"))
    }
}

impl Blockstore for ActorBlockstore {
    type Error = ActorError;

    fn get(&self, cid: &Cid) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(Some(ipld::get(cid)))
    }

    fn put<D>(&self, code: Code, block: &Block<D>) -> Result<Cid, Self::Error>
    where
        D: AsRef<[u8]>,
    {
        // TODO: Don't hard-code the size. Unfortunately, there's no good way to get it from the
        // codec at the moment.
        const SIZE: u32 = 32;
        Ok(ipld::put(
            code.into(),
            SIZE,
            block.codec,
            block.data.as_ref(),
        ))
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error> {
        let k2 = self.put(
            Code::try_from(k.hash().code())
                .map_err(|e| ActorError::new(ExitCode::ErrSerialization, e.to_string()))?,
            &Block::new(k.codec(), block),
        )?;
        if k != &k2 {
            Err(ActorError::new(
                ExitCode::ErrSerialization,
                format!("put block with cid {} but has cid {}", k, k2),
            ))
        } else {
            Ok(())
        }
    }
}
