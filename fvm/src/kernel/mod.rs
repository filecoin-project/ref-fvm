// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
pub use blocks::{Block, BlockId, BlockRegistry, BlockStat};
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::signature::{
    SignatureType, SECP_PUB_LEN, SECP_SIG_LEN, SECP_SIG_MESSAGE_HASH_SIZE,
};
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::piece::PieceInfo;
use fvm_shared::randomness::{Randomness, RANDOMNESS_LENGTH};
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, ReplicaUpdateInfo, SealVerifyInfo,
    WindowPoStVerifyInfo,
};
use fvm_shared::sys::out::network::NetworkContext;
use fvm_shared::sys::out::vm::MessageContext;
use fvm_shared::sys::SendFlags;
use fvm_shared::{ActorID, MethodNum};

mod hash;

mod blocks;
pub mod default;

pub(crate) mod error;

pub use error::{ClassifyResult, Context, ExecutionError, Result, SyscallError};
use fvm_shared::event::StampedEvent;
pub use hash::SupportedHashes;
use multihash::MultihashGeneric;

use crate::call_manager::CallManager;
use crate::gas::{Gas, GasTimer, PriceList};
use crate::machine::limiter::MemoryLimiter;
use crate::machine::Machine;

pub struct SendResult {
    pub block_id: BlockId,
    pub block_stat: BlockStat,
    pub exit_code: ExitCode,
}

/// The "kernel" implements the FVM interface as presented to the actors. It:
///
/// - Manages the Actor's state.
/// - Tracks and charges for IPLD & syscall-specific gas.
///
/// Actors may call into the kernel via the syscalls defined in the [`syscalls`][crate::syscalls]
/// module.
pub trait Kernel:
    ActorOps
    + IpldBlockOps
    + CircSupplyOps
    + CryptoOps
    + DebugOps
    + EventOps
    + GasOps
    + MessageOps
    + NetworkOps
    + RandomnessOps
    + SelfOps
    + SendOps
    + LimiterOps
    + 'static
{
    /// The [`Kernel`]'s [`CallManager`] is
    type CallManager: CallManager;

    /// Consume the [`Kernel`] and return the underlying [`CallManager`] and [`BlockRegistry`].
    fn into_inner(self) -> (Self::CallManager, BlockRegistry)
    where
        Self: Sized;

    /// Construct a new [`Kernel`] from the given [`CallManager`].
    ///
    /// - `caller` is the ID of the _immediate_ caller.
    /// - `actor_id` is the ID of _this_ actor.
    /// - `method` is the method that has been invoked.
    /// - `value_received` is value received due to the current call.
    /// - `blocks` is the initial block registry (should already contain the parameters).
    #[allow(clippy::too_many_arguments)]
    fn new(
        mgr: Self::CallManager,
        blocks: BlockRegistry,
        caller: ActorID,
        actor_id: ActorID,
        method: MethodNum,
        value_received: TokenAmount,
        read_only: bool,
    ) -> Self
    where
        Self: Sized;

    /// The kernel's underlying "machine".
    fn machine(&self) -> &<Self::CallManager as CallManager>::Machine;
}

/// Network-related operations.
pub trait NetworkOps {
    /// Network information (epoch, version, etc.).
    fn network_context(&self) -> Result<NetworkContext>;

    /// The CID of the tipset at the specified epoch.
    fn tipset_cid(&self, epoch: ChainEpoch) -> Result<Cid>;
}

/// Accessors to query attributes of the incoming message.
pub trait MessageOps {
    /// Message information.
    fn msg_context(&self) -> Result<MessageContext>;
}

/// The IPLD subset of the kernel.
pub trait IpldBlockOps {
    /// Open a block.
    ///
    /// This method will fail if the requested block isn't reachable.
    fn block_open(&mut self, cid: &Cid) -> Result<(BlockId, BlockStat)>;

    /// Create a new block.
    ///
    /// This method will fail if the block is too large (SPEC_AUDIT), the codec is not allowed
    /// (SPEC_AUDIT), the block references unreachable blocks, or the block contains too many links
    /// (SPEC_AUDIT).
    fn block_create(&mut self, codec: u64, data: &[u8]) -> Result<BlockId>;

    /// Computes a CID for a block.
    ///
    /// This is the only way to add a new block to the "reachable" set.
    ///
    /// This method will fail if the block handle is invalid.
    fn block_link(&mut self, id: BlockId, hash_fun: u64, hash_len: u32) -> Result<Cid>;

    /// Read data from a block.
    ///
    /// This method will fail if the block handle is invalid.
    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<i32>;

    /// Returns the blocks codec & size.
    ///
    /// This method will fail if the block handle is invalid.
    fn block_stat(&self, id: BlockId) -> Result<BlockStat>;
}

/// Actor state access and manipulation.
/// Depends on BlockOps to read and write blocks in the state tree.
pub trait SelfOps: IpldBlockOps {
    /// Get the state root.
    fn root(&self) -> Result<Cid>;

    /// Update the state-root.
    ///
    /// This method will fail if the new state-root isn't reachable.
    fn set_root(&mut self, root: Cid) -> Result<()>;

    /// The balance of the receiver.
    fn current_balance(&self) -> Result<TokenAmount>;

    /// Deletes the executing actor from the state tree, transferring any balance to beneficiary.
    /// Aborts if the beneficiary does not exist.
    /// May only be called by the actor itself.
    fn self_destruct(&mut self, beneficiary: &Address) -> Result<()>;
}

/// Actors operations whose scope of action is actors other than the calling
/// actor. The calling actor's state may be consulted to resolve some.
pub trait ActorOps {
    /// Resolves an address of any protocol to an ID address (via the Init actor's table).
    /// This allows resolution of externally-provided SECP, BLS, or actor addresses to the canonical form.
    /// If the argument is an ID address it is returned directly.
    fn resolve_address(&self, address: &Address) -> Result<ActorID>;

    /// Looks up the "delegated" (f4) address of the specified actor, if any.
    fn lookup_delegated_address(&self, actor_id: ActorID) -> Result<Option<Address>>;

    /// Look up the code CID of an actor.
    fn get_actor_code_cid(&self, id: ActorID) -> Result<Cid>;

    /// Computes an address for a new actor. The returned address is intended to uniquely refer to
    /// the actor even in the event of a chain re-org (whereas an ID-address might refer to a
    /// different actor after messages are re-ordered).
    /// Always an ActorExec address.
    fn next_actor_address(&self) -> Result<Address>;

    /// Creates an actor with given `code_cid`, `actor_id`, `delegated_address` (if specified),
    /// and an empty state.
    fn create_actor(
        &mut self,
        code_cid: Cid,
        actor_id: ActorID,
        delegated_address: Option<Address>,
    ) -> Result<()>;

    /// Installs actor code pointed by cid
    #[cfg(feature = "m2-native")]
    fn install_actor(&mut self, code_cid: Cid) -> Result<()>;

    /// Returns the actor's "type" (if builitin) or 0 (if not).
    fn get_builtin_actor_type(&self, code_cid: &Cid) -> Result<u32>;

    /// Returns the CodeCID for the supplied built-in actor type.
    fn get_code_cid_for_type(&self, typ: u32) -> Result<Cid>;

    /// Returns the balance associated with an actor id
    fn balance_of(&self, actor_id: ActorID) -> Result<TokenAmount>;
}

/// Operations to send messages to other actors.
pub trait SendOps {
    fn send(
        &mut self,
        recipient: &Address,
        method: u64,
        params: BlockId,
        value: &TokenAmount,
        gas_limit: Option<Gas>,
        flags: SendFlags,
    ) -> Result<SendResult>;
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
    fn total_fil_circ_supply(&self) -> Result<TokenAmount>;
}

/// Operations for explicit gas charging.
pub trait GasOps {
    /// Returns the gas used by the transaction so far.
    fn gas_used(&self) -> Gas;

    /// Returns the remaining gas for the transaction.
    fn gas_available(&self) -> Gas;

    /// ChargeGas charges specified amount of `gas` for execution.
    /// `name` provides information about gas charging point.
    fn charge_gas(&self, name: &str, compute: Gas) -> Result<GasTimer>;

    /// Returns the currently active gas price list.
    fn price_list(&self) -> &PriceList;
}

/// Cryptographic primitives provided by the kernel.
pub trait CryptoOps {
    /// Verifies that a signature is valid for an address and plaintext.
    fn verify_signature(
        &self,
        sig_type: SignatureType,
        signature: &[u8],
        signer: &Address,
        plaintext: &[u8],
    ) -> Result<bool>;

    /// Given a message hash and its signature, recovers the public key of the signer.
    fn recover_secp_public_key(
        &self,
        hash: &[u8; SECP_SIG_MESSAGE_HASH_SIZE],
        signature: &[u8; SECP_SIG_LEN],
    ) -> Result<[u8; SECP_PUB_LEN]>;

    /// Hashes input `data_in` using with the specified hash function, writing the output to
    /// `digest_out`, returning the size of the digest written to `digest_out`. If `digest_out` is
    /// to small to fit the entire digest, it will be truncated. If too large, the leftover space
    /// will not be overwritten.
    fn hash(&self, code: u64, data: &[u8]) -> Result<MultihashGeneric<64>>;

    /// Computes an unsealed sector CID (CommD) from its constituent piece CIDs (CommPs) and sizes.
    fn compute_unsealed_sector_cid(
        &self,
        proof_type: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Result<Cid>;

    /// Verifies a sector seal proof.
    fn verify_seal(&self, vi: &SealVerifyInfo) -> Result<bool>;

    /// Verifies a window proof of spacetime.
    fn verify_post(&self, verify_info: &WindowPoStVerifyInfo) -> Result<bool>;

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
    ) -> Result<Option<ConsensusFault>>;

    /// Verifies a batch of seals. This is a privledged syscall, may _only_ be called by the
    /// power actor during cron.
    ///
    /// Gas: This syscall intentionally _does not_ charge any gas (as said gas would be charged to
    /// cron). Instead, gas is pre-paid by the storage provider on pre-commit.
    fn batch_verify_seals(&self, vis: &[SealVerifyInfo]) -> Result<Vec<bool>>;

    /// Verify aggregate seals verifies an aggregated batch of prove-commits.
    fn verify_aggregate_seals(&self, aggregate: &AggregateSealVerifyProofAndInfos) -> Result<bool>;

    /// Verify replica update verifies a snap deal: an upgrade from a CC sector to a sector with
    /// deals.
    fn verify_replica_update(&self, replica: &ReplicaUpdateInfo) -> Result<bool>;
}

/// Randomness queries.
pub trait RandomnessOps {
    /// Randomness returns a (pseudo)random byte array drawing from the latest
    /// ticket chain from a given epoch and incorporating requisite entropy.
    /// This randomness is fork dependant but also biasable because of this.
    fn get_randomness_from_tickets(
        &self,
        personalization: i64,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<[u8; RANDOMNESS_LENGTH]>;

    /// Randomness returns a (pseudo)random byte array drawing from the latest
    /// beacon from a given epoch and incorporating requisite entropy.
    /// This randomness is not tied to any fork of the chain, and is unbiasable.
    fn get_randomness_from_beacon(
        &self,
        personalization: i64,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<[u8; RANDOMNESS_LENGTH]>;
}

/// Debugging APIs.
pub trait DebugOps {
    /// Log a message.
    fn log(&self, msg: String);

    /// Returns whether debug mode is enabled.
    fn debug_enabled(&self) -> bool;

    /// Store an artifact.
    /// Returns error on malformed name, returns Ok and logs the error on system/os errors.
    fn store_artifact(&self, name: &str, data: &[u8]) -> Result<()>;
}

/// Track and limit memory expansion.
///
/// This interface is not one of the operations the kernel provides to actors.
/// It's only part of the kernel out of necessity to pass it through to the
/// call manager which tracks the limits across the whole execution stack.
pub trait LimiterOps {
    type Limiter: MemoryLimiter;
    /// Give access to the limiter of the underlying call manager.
    fn limiter_mut(&mut self) -> &mut Self::Limiter;
}

/// Eventing APIs.
pub trait EventOps {
    /// Records an event emitted throughout execution.
    fn emit_event(&mut self, raw_evt: &[u8]) -> Result<()>;
}
