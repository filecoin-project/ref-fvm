// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::blockstore::Blockstore;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{de, Cbor, RawBytes};
use fvm_shared::error::ExitCode;
use fvm_shared::piece::PieceInfo;
use fvm_shared::randomness::Randomness;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, SealVerifyInfo, WindowPoStVerifyInfo,
};
use fvm_shared::version::NetworkVersion;
use fvm_shared::{ActorID, MethodNum};

pub use self::actor_code::*;
use crate::ActorError;

mod actor_code;

#[cfg(feature = "runtime-wasm")]
pub mod fvm;

#[cfg(feature = "runtime-wasm")]
mod actor_blockstore;

/// Runtime is the VM's internal runtime object.
/// this is everything that is accessible to actors, beyond parameters.
pub trait Runtime<BS: Blockstore>: Syscalls {
    /// The network protocol version number at the current epoch.
    fn network_version(&self) -> NetworkVersion;

    /// Information related to the current message being executed.
    fn message(&self) -> &dyn MessageInfo;

    /// The current chain epoch number. The genesis block has epoch zero.
    fn curr_epoch(&self) -> ChainEpoch;

    /// Validates the caller against some predicate.
    /// Exported actor methods must invoke at least one caller validation before returning.
    fn validate_immediate_caller_accept_any(&mut self) -> Result<(), ActorError>;
    fn validate_immediate_caller_is<'a, I>(&mut self, addresses: I) -> Result<(), ActorError>
    where
        I: IntoIterator<Item = &'a Address>;
    fn validate_immediate_caller_type<'a, I>(&mut self, types: I) -> Result<(), ActorError>
    where
        I: IntoIterator<Item = &'a Cid>;

    /// The balance of the receiver.
    fn current_balance(&self) -> TokenAmount;

    /// Resolves an address of any protocol to an ID address (via the Init actor's table).
    /// This allows resolution of externally-provided SECP, BLS, or actor addresses to the canonical form.
    /// If the argument is an ID address it is returned directly.
    fn resolve_address(&self, address: &Address) -> Option<Address>;

    /// Look up the code ID at an actor address.
    fn get_actor_code_cid(&self, addr: &Address) -> Option<Cid>;

    /// Randomness returns a (pseudo)random byte array drawing from the latest
    /// ticket chain from a given epoch and incorporating requisite entropy.
    /// This randomness is fork dependant but also biasable because of this.
    fn get_randomness_from_tickets(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<Randomness, ActorError>;

    /// Randomness returns a (pseudo)random byte array drawing from the latest
    /// beacon from a given epoch and incorporating requisite entropy.
    /// This randomness is not tied to any fork of the chain, and is unbiasable.
    fn get_randomness_from_beacon(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<Randomness, ActorError>;

    /// Initializes the state object.
    /// This is only valid in a constructor function and when the state has not yet been initialized.
    fn create<C: Cbor>(&mut self, obj: &C) -> Result<(), ActorError>;

    /// Loads a readonly copy of the state of the receiver into the argument.
    ///
    /// Any modification to the state is illegal and will result in an abort.
    fn state<C: Cbor>(&self) -> Result<C, ActorError>;

    /// Loads a mutable version of the state into the `obj` argument and protects
    /// the execution from side effects (including message send).
    ///
    /// The second argument is a function which allows the caller to mutate the state.
    /// The return value from that function will be returned from the call to Transaction().
    ///
    /// If the state is modified after this function returns, execution will abort.
    ///
    /// The gas cost of this method is that of a Store.Put of the mutated state object.
    fn transaction<C, RT, F>(&mut self, f: F) -> Result<RT, ActorError>
    where
        C: Cbor,
        F: FnOnce(&mut C, &mut Self) -> Result<RT, ActorError>;

    /// Returns reference to blockstore
    fn store(&self) -> &BS;

    /// Sends a message to another actor, returning the exit code and return value envelope.
    /// If the invoked method does not return successfully, its state changes
    /// (and that of any messages it sent in turn) will be rolled back.
    fn send(
        &self,
        to: Address,
        method: MethodNum,
        params: RawBytes,
        value: TokenAmount,
    ) -> Result<RawBytes, ActorError>;

    /// Computes an address for a new actor. The returned address is intended to uniquely refer to
    /// the actor even in the event of a chain re-org (whereas an ID-address might refer to a
    /// different actor after messages are re-ordered).
    /// Always an ActorExec address.
    fn new_actor_address(&mut self) -> Result<Address, ActorError>;

    /// Creates an actor with code `codeID` and address `address`, with empty state.
    /// May only be called by Init actor.
    fn create_actor(&mut self, code_id: Cid, address: ActorID) -> Result<(), ActorError>;

    /// Deletes the executing actor from the state tree, transferring any balance to beneficiary.
    /// Aborts if the beneficiary does not exist.
    /// May only be called by the actor itself.
    fn delete_actor(&mut self, beneficiary: &Address) -> Result<(), ActorError>;

    /// Returns the total token supply in circulation at the beginning of the current epoch.
    /// The circulating supply is the sum of:
    /// - rewards emitted by the reward actor,
    /// - funds vested from lock-ups in the genesis state,
    /// less the sum of:
    /// - funds burnt,
    /// - pledge collateral locked in storage miner actors (recorded in the storage power actor)
    /// - deal collateral locked by the storage market actor
    fn total_fil_circ_supply(&self) -> TokenAmount;

    /// ChargeGas charges specified amount of `gas` for execution.
    /// `name` provides information about gas charging point
    fn charge_gas(&mut self, name: &'static str, compute: i64);

    /// This function is a workaround for go-implementation's faulty exit code handling of
    /// parameters before version 7
    fn deserialize_params<O: de::DeserializeOwned>(
        &self,
        params: &RawBytes,
    ) -> Result<O, ActorError> {
        params.deserialize().map_err(|e| {
            if self.network_version() < NetworkVersion::V7 {
                ActorError::new(
                    ExitCode::SysErrSenderInvalid,
                    format!("failed to decode parameters: {}", e),
                )
            } else {
                ActorError::from(e).wrap("failed to decode parameters")
            }
        })
    }

    fn base_fee(&self) -> TokenAmount;
}

/// Message information available to the actor about executing message.
pub trait MessageInfo {
    /// The address of the immediate calling actor. Always an ID-address.
    fn caller(&self) -> Address;

    /// The address of the actor receiving the message. Always an ID-address.
    fn receiver(&self) -> Address;

    /// The value attached to the message being processed, implicitly
    /// added to current_balance() before method invocation.
    fn value_received(&self) -> TokenAmount;
}

/// Pure functions implemented as primitives by the runtime.
pub trait Syscalls {
    /// Verifies that a signature is valid for an address and plaintext.
    fn verify_signature(
        &self,
        signature: &Signature,
        signer: &Address,
        plaintext: &[u8],
    ) -> Result<(), anyhow::Error>;

    /// Hashes input data using blake2b with 256 bit output.
    fn hash_blake2b(&self, data: &[u8]) -> [u8; 32];

    /// Computes an unsealed sector CID (CommD) from its constituent piece CIDs (CommPs) and sizes.
    fn compute_unsealed_sector_cid(
        &self,
        proof_type: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Result<Cid, anyhow::Error>;

    /// Verifies a sector seal proof.
    fn verify_seal(&self, vi: &SealVerifyInfo) -> Result<(), anyhow::Error>;

    /// Verifies a window proof of spacetime.
    fn verify_post(&self, verify_info: &WindowPoStVerifyInfo) -> Result<(), anyhow::Error>;

    /// Verifies that two block headers provide proof of a consensus fault:
    /// - both headers mined by the same actor
    /// - headers are different
    /// - first header is of the same or lower epoch as the second
    /// - at least one of the headers appears in the current chain at or after epoch `earliest`
    /// - the headers provide evidence of a fault (see the spec for the different fault types).
    /// The parameters are all serialized block headers. The third "extra" parameter is consulted only for
    /// the "parent grinding fault", in which case it must be the sibling of h1 (same parent tipset) and one of the
    /// blocks in the parent of h2 (i.e. h2's grandparent).
    /// Returns nil and an error if the headers don't prove a fault.
    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> Result<Option<ConsensusFault>, anyhow::Error>;

    fn batch_verify_seals(&self, batch: &[SealVerifyInfo]) -> anyhow::Result<Vec<bool>> {
        Ok(batch
            .iter()
            .map(|si| self.verify_seal(si).is_ok())
            .collect())
    }
    fn verify_aggregate_seals(
        &self,
        aggregate: &AggregateSealVerifyProofAndInfos,
    ) -> Result<(), anyhow::Error>;
}
