use std::collections::HashMap;

use cid::Cid;

use crate::message::Message;
pub use blocks::{BlockError, BlockId, BlockStat};
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::RawBytes;
use fvm_shared::error::ActorError;
use fvm_shared::piece::PieceInfo;
use fvm_shared::randomness::Randomness;
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, SealVerifyInfo, WindowPoStVerifyInfo,
};
use fvm_shared::version::NetworkVersion;
use fvm_shared::{ActorID, MethodNum};
pub use mapcell::*;

mod blocks;
pub mod default;
mod mapcell;

// Type aliases to make return values easier to follow.
// TODO ActorError should be replaced with SystemError (or similar), as these
//  are _not_ actor errors.
type Fallible<T, E = ActorError> = anyhow::Result<T, E>;
type Infallible<T> = T;

pub trait Kernel:
    ActorOps
    + BlockOps
    + CircSupplyOps
    + CryptoOps
    + GasOps
    + MessageOps
    + NetworkOps
    + RandomnessOps
    + ReturnOps
    + SelfOps
    + SendOps
    + ValidationOps
{
}

// TODO @raulk: most of these methods should NOT generate an ActorError, since
//  the errors raised by the impls of these traits are system errors. We need to
//  segregate the monolithic ActorError into a system error, actor error, and more.

/// Network-related operations.
pub trait NetworkOps {
    fn network_curr_epoch(&self) -> Infallible<ChainEpoch>;
    fn network_version(&self) -> Infallible<NetworkVersion>;
    fn network_base_fee(&self) -> Infallible<&TokenAmount>;
}

/// Message validation operations.
/// Exported actor methods must invoke at least one caller validation before returning.
///
/// TODO Kernel must track validation status.
pub trait ValidationOps {
    fn validate_immediate_caller_accept_any(&mut self) -> Fallible<()>;
    fn validate_immediate_caller_addr_one_of(&mut self, allowed: Vec<Address>) -> Fallible<()>;
    fn validate_immediate_caller_type_one_of(&mut self, allowed: Vec<Cid>) -> Fallible<()>;
}

/// Accessors to query attributes of the incoming message.
pub trait MessageOps {
    fn msg_caller(&self) -> ActorID;
    fn msg_receiver(&self) -> ActorID;
    fn msg_method_number(&self) -> MethodNum;
    fn msg_method_params(&self) -> BlockId;
    fn msg_value_received(&self) -> u128;
}

/// The IPLD subset of the kernel.
pub trait BlockOps {
    /// Open a block.
    ///
    /// This method will fail if the requested block isn't reachable.
    fn block_open(&mut self, cid: &Cid) -> Fallible<BlockId, BlockError>;

    /// Create a new block.
    ///
    /// This method will fail if the block is too large (SPEC_AUDIT), the codec is not allowed
    /// (SPEC_AUDIT), the block references unreachable blocks, or the block contains too many links
    /// (SPEC_AUDIT).
    fn block_create(&mut self, codec: u64, data: &[u8]) -> Fallible<BlockId, BlockError>;

    /// Computes a CID for a block.
    ///
    /// This is the only way to add a new block to the "reachable" set.
    ///
    /// This method will fail if the block handle is invalid.
    fn block_link(
        &mut self,
        id: BlockId,
        hash_fun: u64,
        hash_len: u32,
    ) -> Fallible<Cid, BlockError>;

    /// Read data from a block.
    ///
    /// This method will fail if the block handle is invalid.
    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Fallible<u32, BlockError>;

    /// Returns the blocks codec & size.
    ///
    /// This method will fail if the block handle is invalid.
    fn block_stat(&self, id: BlockId) -> Fallible<BlockStat, BlockError>;

    // TODO: add a way to _flush_ new blocks.
}

/// Actor state access and manipulation.
/// Depends on BlockOps to read and write blocks in the state tree.
pub trait SelfOps: BlockOps {
    /// Get the state root.
    fn root(&self) -> Cid;

    /// Update the state-root.
    ///
    /// This method will fail if the new state-root isn't reachable.
    fn set_root(&mut self, root: Cid) -> Fallible<()>;

    /// The balance of the receiver.
    fn current_balance(&self) -> Fallible<TokenAmount>;

    /// Deletes the executing actor from the state tree, transferring any balance to beneficiary.
    /// Aborts if the beneficiary does not exist.
    /// May only be called by the actor itself.
    fn self_destruct(&mut self, beneficiary: &Address) -> Fallible<()>;
}

/// Actors operations whose scope of action is actors other than the calling
/// actor. The calling actor's state may be consulted to resolve some.
pub trait ActorOps {
    /// Resolves an address of any protocol to an ID address (via the Init actor's table).
    /// This allows resolution of externally-provided SECP, BLS, or actor addresses to the canonical form.
    /// If the argument is an ID address it is returned directly.
    fn resolve_address(&self, address: &Address) -> Fallible<Option<Address>>;

    /// Look up the code ID at an actor address.
    fn get_actor_code_cid(&self, addr: &Address) -> Fallible<Option<Cid>>;

    /// Computes an address for a new actor. The returned address is intended to uniquely refer to
    /// the actor even in the event of a chain re-org (whereas an ID-address might refer to a
    /// different actor after messages are re-ordered).
    /// Always an ActorExec address.
    fn new_actor_address(&mut self) -> Fallible<Address>;

    /// Creates an actor with code `codeID` and address `address`, with empty state.
    /// May only be called by Init actor.
    fn create_actor(&mut self, code_id: Cid, address: &Address) -> Fallible<()>;
}

/// Operations that query and manipulate the return stack. The return stack is
/// how the kernel delivers variable-length return values to the caller.
pub trait ReturnOps {
    /// Returns the size of the top element in the return stack.
    /// 0 means non-existent, otherwise the length is returned.
    fn return_size(&self) -> u64;

    /// Discards the top element in the return stack.
    fn return_discard(&mut self);

    /// Pops the top element off the return stack, and copies it into the
    /// specified buffer. This buffer must be appropriately sized according to
    /// return_size. This method returns the amount of bytes copied.
    fn return_pop(&mut self, into: &mut [u8]) -> u64;
}

/// Operations to send messages to other actors.
pub trait SendOps {
    fn send(&mut self, message: Message) -> Fallible<RawBytes>;
}

/// Operations to query the circulating supply.
pub trait CircSupplyOps {
    /// Returns the total token supply in circulation at the beginning of the current epoch.
    /// The circulating supply is the sum of:
    /// - rewards emitted by the reward actor,
    /// - funds vested from lock-ups in the genesis state,
    /// less the sum of:
    /// - funds burnt,
    /// - pledge collateral locked in storage miner actors (recorded in the storage power actor)
    /// - deal collateral locked by the storage market actor
    fn total_fil_circ_supply(&self) -> Fallible<TokenAmount>;
}

/// Operations for explicit gas charging.
///
/// TODO this is unsafe; most gas charges should occur as part of syscalls, but
///  some built-in actors currently charge gas explicitly for concrete actions.
///  In the future (M1), this should disappear and be replaced by gas instrumentation
///  at the WASM level.
pub trait GasOps {
    /// ChargeGas charges specified amount of `gas` for execution.
    /// `name` provides information about gas charging point
    fn charge_gas(&mut self, name: &'static str, compute: i64) -> Fallible<()>;
}

/// Cryptographic primitives provided by the kernel.
pub trait CryptoOps {
    /// Verifies that a signature is valid for an address and plaintext.
    fn verify_signature(
        &self,
        signature: &Signature,
        signer: &Address,
        plaintext: &[u8],
    ) -> Fallible<()>;

    /// Hashes input data using blake2b with 256 bit output.
    fn hash_blake2b(&self, data: &[u8]) -> Fallible<[u8; 32]>;

    /// Computes an unsealed sector CID (CommD) from its constituent piece CIDs (CommPs) and sizes.
    fn compute_unsealed_sector_cid(
        &self,
        proof_type: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Fallible<Cid>;

    /// Verifies a sector seal proof.
    fn verify_seal(&self, vi: &SealVerifyInfo) -> Fallible<()>;

    /// Verifies a window proof of spacetime.
    fn verify_post(&self, verify_info: &WindowPoStVerifyInfo) -> Fallible<()>;

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
    ) -> Fallible<Option<ConsensusFault>>;

    fn batch_verify_seals(
        &self,
        vis: &[(&Address, &Vec<SealVerifyInfo>)],
    ) -> Fallible<HashMap<Address, Vec<bool>>>;

    fn verify_aggregate_seals(&self, aggregate: &AggregateSealVerifyProofAndInfos) -> Fallible<()>;
}

/// Randomness queries.
pub trait RandomnessOps {
    /// Randomness returns a (pseudo)random byte array drawing from the latest
    /// ticket chain from a given epoch and incorporating requisite entropy.
    /// This randomness is fork dependant but also biasable because of this.
    fn get_randomness_from_tickets(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Fallible<Randomness>;

    /// Randomness returns a (pseudo)random byte array drawing from the latest
    /// beacon from a given epoch and incorporating requisite entropy.
    /// This randomness is not tied to any fork of the chain, and is unbiasable.
    fn get_randomness_from_beacon(
        &self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Fallible<Randomness>;
}
