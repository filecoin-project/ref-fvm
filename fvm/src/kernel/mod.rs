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

mod blocks;
mod hash;

pub mod default;
pub mod filecoin;

pub(crate) mod error;

use ambassador::delegatable_trait;
pub use error::{ClassifyResult, Context, ExecutionError, Result, SyscallError};
use fvm_shared::event::StampedEvent;
pub use hash::SupportedHashes;
use multihash::MultihashGeneric;
use wasmtime::Linker;

use crate::call_manager::CallManager;
use crate::gas::{Gas, GasTimer, PriceList};
use crate::machine::limiter::MemoryLimiter;
use crate::machine::Machine;
use crate::syscalls::InvocationData;

pub struct CallResult {
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
    SyscallHandler<Self>
    + ActorOps
    + IpldBlockOps
    + CryptoOps
    + DebugOps
    + EventOps
    + GasOps
    + MessageOps
    + NetworkOps
    + RandomnessOps
    + SelfOps
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

    /// Sends a message to another actor.
    /// The method type parameter K is the type of the kernel to instantiate for
    /// the receiving actor. This is necessary to support wrapping a kernel, so the outer
    /// kernel can specify its Self as the receiver's kernel type, rather than the wrapped
    /// kernel specifying its Self.
    /// This method is part of the Kernel trait so it can refer to the Self::CallManager
    /// associated type necessary to constrain K.
    fn send<K: Kernel<CallManager = Self::CallManager>>(
        &mut self,
        recipient: &Address,
        method: u64,
        params: BlockId,
        value: &TokenAmount,
        gas_limit: Option<Gas>,
        flags: SendFlags,
    ) -> Result<CallResult>;

    fn upgrade_actor<K: Kernel<CallManager = Self::CallManager>>(
        &mut self,
        new_code_cid: Cid,
        params_id: BlockId,
    ) -> Result<CallResult>;
}

pub trait SyscallHandler<K: Kernel>: Sized {
    fn bind_syscalls(&self, linker: &mut Linker<InvocationData<K>>) -> anyhow::Result<()>;
}

/// Network-related operations.
#[delegatable_trait]
pub trait NetworkOps {
    /// Network information (epoch, version, etc.).
    fn network_context(&self) -> Result<NetworkContext>;

    /// The CID of the tipset at the specified epoch.
    fn tipset_cid(&self, epoch: ChainEpoch) -> Result<Cid>;
}

/// Accessors to query attributes of the incoming message.
#[delegatable_trait]
pub trait MessageOps {
    /// Message information.
    fn msg_context(&self) -> Result<MessageContext>;
}

/// The IPLD subset of the kernel.
#[delegatable_trait]
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
#[delegatable_trait]
pub trait SelfOps: IpldBlockOps {
    /// Get the state root.
    fn root(&mut self) -> Result<Cid>;

    /// Update the state-root.
    ///
    /// This method will fail if the new state-root isn't reachable.
    fn set_root(&mut self, root: Cid) -> Result<()>;

    /// The balance of the receiver.
    fn current_balance(&self) -> Result<TokenAmount>;

    /// Deletes the executing actor from the state tree, burning any remaining balance if requested.
    fn self_destruct(&mut self, burn_unspent: bool) -> Result<()>;
}

/// Actors operations whose scope of action is actors other than the calling
/// actor. The calling actor's state may be consulted to resolve some.
#[delegatable_trait]
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

    fn install_actor(&mut self, code_cid: Cid) -> Result<()>;

    /// Returns the actor's "type" (if builitin) or 0 (if not).
    fn get_builtin_actor_type(&self, code_cid: &Cid) -> Result<u32>;

    /// Returns the CodeCID for the supplied built-in actor type.
    fn get_code_cid_for_type(&self, typ: u32) -> Result<Cid>;

    /// Returns the balance associated with an actor id
    fn balance_of(&self, actor_id: ActorID) -> Result<TokenAmount>;
}

/// Operations for explicit gas charging.
#[delegatable_trait]
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
#[delegatable_trait]
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
}

/// Randomness queries.
#[delegatable_trait]
pub trait RandomnessOps {
    /// Randomness returns a (pseudo)random byte array drawing from the latest
    /// ticket chain from a given epoch.
    /// This randomness is fork dependant but also biasable because of this.
    fn get_randomness_from_tickets(
        &self,
        rand_epoch: ChainEpoch,
    ) -> Result<[u8; RANDOMNESS_LENGTH]>;

    /// Randomness returns a (pseudo)random byte array drawing from the latest
    /// beacon from a given epoch.
    /// This randomness is not tied to any fork of the chain, and is unbiasable.
    fn get_randomness_from_beacon(&self, rand_epoch: ChainEpoch)
        -> Result<[u8; RANDOMNESS_LENGTH]>;
}

/// Debugging APIs.
#[delegatable_trait]
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
#[delegatable_trait]
pub trait LimiterOps {
    type Limiter: MemoryLimiter;
    /// Give access to the limiter of the underlying call manager.
    fn limiter_mut(&mut self) -> &mut Self::Limiter;
}

/// Eventing APIs.
#[delegatable_trait]
pub trait EventOps {
    /// Records an event emitted throughout execution.
    fn emit_event(
        &mut self,
        event_headers: &[fvm_shared::sys::EventEntry],
        raw_key: &[u8],
        raw_val: &[u8],
    ) -> Result<()>;
}
