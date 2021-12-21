use anyhow::Error;
use cid::{multihash::Code, Cid};
use fvm_sdk::ipld;
use fvm_shared::error::{ActorError, ExitCode};
use std::convert::TryFrom;

use crate::runtime::{ConsensusFault, MessageInfo, Syscalls};
use crate::Runtime;
use blockstore::{Block, Blockstore};
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{Cbor, RawBytes};
use fvm_shared::randomness::Randomness;
use fvm_shared::sector::{AggregateSealVerifyProofAndInfos, SealVerifyInfo, WindowPoStVerifyInfo};
use fvm_shared::version::NetworkVersion;
use fvm_shared::MethodNum;

/// A blockstore suitable for use within actors.
pub struct ActorBlockstore;

pub struct SdkRuntime;

impl<B> Runtime<B> for SdkRuntime {
    fn network_version(&self) -> NetworkVersion {
        todo!()
    }

    fn message(&self) -> &dyn MessageInfo {
        todo!()
    }

    fn curr_epoch(&self) -> ChainEpoch {
        todo!()
    }

    fn validate_immediate_caller_accept_any(&mut self) -> Result<(), ActorError> {
        todo!()
    }

    fn validate_immediate_caller_is<'a, I>(&mut self, addresses: I) -> Result<(), ActorError>
    where
        I: IntoIterator<Item = &'a Address>,
    {
        todo!()
    }

    fn validate_immediate_caller_type<'a, I>(&mut self, types: I) -> Result<(), ActorError>
    where
        I: IntoIterator<Item = &'a Cid>,
    {
        todo!()
    }

    fn current_balance(&self) -> Result<TokenAmount, ActorError> {
        todo!()
    }

    fn resolve_address(&self, address: &Address) -> Result<Option<Address>, ActorError> {
        todo!()
    }

    fn get_actor_code_cid(&self, addr: &Address) -> Result<Option<Cid>, ActorError> {
        todo!()
    }

    fn get_randomness_from_tickets(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<Randomness, ActorError> {
        todo!()
    }

    fn get_randomness_from_beacon(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<Randomness, ActorError> {
        fvm_sdk::rand::get_beacon_randomness(personalization, rand_epoch, entropy)
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

    fn store(&self) -> &ß {
        todo!()
    }

    fn send(
        &mut self,
        to: Address,
        method: MethodNum,
        params: RawBytes,
        value: TokenAmount,
    ) -> Result<RawBytes, ActorError> {
        todo!()
    }

    fn new_actor_address(&mut self) -> Result<Address, ActorError> {
        todo!()
    }

    fn create_actor(&mut self, code_id: Cid, address: &Address) -> Result<(), ActorError> {
        todo!()
    }

    fn delete_actor(&mut self, beneficiary: &Address) -> Result<(), ActorError> {
        todo!()
    }

    fn total_fil_circ_supply(&self) -> Result<TokenAmount, ActorError> {
        todo!()
    }

    fn charge_gas(&mut self, name: &'static str, compute: i64) -> Result<(), ActorError> {
        todo!()
    }

    fn base_fee(&self) -> &TokenAmount {
        &fvm_sdk::network::base_fee()
    }
}

impl Syscalls for SdkRuntime {
    fn verify_signature(
        &self,
        signature: &Signature,
        signer: &Address,
        plaintext: &[u8],
    ) -> Result<(), Error> {
        todo!()
    }

    fn verify_seal(&self, vi: &SealVerifyInfo) -> Result<(), Error> {
        todo!()
    }

    fn verify_post(&self, verify_info: &WindowPoStVerifyInfo) -> Result<(), Error> {
        todo!()
    }

    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> Result<Option<ConsensusFault>, Error> {
        todo!()
    }

    fn verify_aggregate_seals(
        &self,
        aggregate: &AggregateSealVerifyProofAndInfos,
    ) -> Result<(), Error> {
        todo!()
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
