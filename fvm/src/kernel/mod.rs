// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use ambassador::delegatable_trait;
use fvm_shared::event::StampedEvent;

use crate::call_manager::CallManager;
use crate::machine::limiter::MemoryLimiter;
use crate::machine::Machine;
use crate::syscalls::Linker;

mod blocks;
mod error;
mod hash;

pub mod default;
pub mod filecoin;

pub use blocks::{Block, BlockId, BlockRegistry, BlockStat};
pub use error::{ClassifyResult, Context, ExecutionError, Result, SyscallError};
pub use hash::SupportedHashes;

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
pub trait Kernel: SyscallHandler<Self> + 'static {
    /// The [`Kernel`]'s [`CallManager`] is
    type CallManager: CallManager;
    /// The [`Kernel`]'s memory allocation tracker.
    type Limiter: MemoryLimiter;

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

    /// Give access to the limiter of the underlying call manager.
    fn limiter_mut(&mut self) -> &mut Self::Limiter;

    /// Returns the remaining gas for the transaction.
    fn gas_available(&self) -> Gas;

    /// ChargeGas charges specified amount of `gas` for execution.
    /// `name` provides information about gas charging point.
    fn charge_gas(&self, name: &str, compute: Gas) -> Result<GasTimer>;
}

pub trait SyscallHandler<K>: Sized {
    fn link_syscalls(linker: &mut Linker<K>) -> anyhow::Result<()>;
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

/// The actor calling operations.
#[delegatable_trait]
pub trait SendOps<K: Kernel = Self> {
    /// Sends a message to another actor.
    /// The method type parameter K is the type of the kernel to instantiate for
    /// the receiving actor. This is necessary to support wrapping a kernel, so the outer
    /// kernel can specify its Self as the receiver's kernel type, rather than the wrapped
    /// kernel specifying its Self.
    /// This method is part of the Kernel trait so it can refer to the Self::CallManager
    /// associated type necessary to constrain K.
    fn send(
        &mut self,
        recipient: &Address,
        method: u64,
        params: BlockId,
        value: &TokenAmount,
        gas_limit: Option<Gas>,
        flags: SendFlags,
    ) -> Result<CallResult>;
}

/// The actor upgrade operations.
#[delegatable_trait]
pub trait UpgradeOps<K: Kernel = Self> {
    /// Upgrades the running actor to the specified code CID.
    fn upgrade_actor(&mut self, new_code_cid: Cid, params_id: BlockId) -> Result<CallResult>;
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

/// Cryptographic primitives provided by the kernel.
#[delegatable_trait]
pub trait CryptoOps {
    /// Verifies that a signature is valid for an address and plaintext.
    #[cfg(feature = "verify-signature")]
    fn verify_signature(
        &self,
        sig_type: SignatureType,
        signature: &[u8],
        signer: &Address,
        plaintext: &[u8],
    ) -> Result<bool>;

    /// Verifies a BLS aggregate signature. In the case where there is one signer/signed plaintext,
    /// this is equivalent to verifying a non-aggregated BLS signature.
    ///
    /// Returns:
    /// - `Ok(true)` on a valid signature.
    /// - `Ok(false)` on an invalid signature or if the signature or public keys' bytes represent an
    ///    invalid curve point.
    /// - `Err(IllegalArgument)` if `pub_keys.len() != plaintexts.len()`.
    fn verify_bls_aggregate(
        &self,
        aggregate_sig: &[u8; fvm_shared::crypto::signature::BLS_SIG_LEN],
        pub_keys: &[[u8; fvm_shared::crypto::signature::BLS_PUB_LEN]],
        plaintexts_concat: &[u8],
        plaintext_lens: &[u32],
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
    fn hash(&self, code: u64, data: &[u8]) -> Result<Multihash>;
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

/// Import this module (with a glob) if you're implementing a kernel, _especially_ if you want to
/// use ambassador to delegate the implementation.
pub mod prelude {
    pub use super::{
        ambassador_impl_ActorOps, ambassador_impl_CryptoOps, ambassador_impl_DebugOps,
        ambassador_impl_EventOps, ambassador_impl_IpldBlockOps, ambassador_impl_MessageOps,
        ambassador_impl_NetworkOps, ambassador_impl_RandomnessOps, ambassador_impl_SelfOps,
        ambassador_impl_SendOps, ambassador_impl_UpgradeOps,
    };
    pub use super::{
        ActorOps, CryptoOps, DebugOps, EventOps, IpldBlockOps, MessageOps, NetworkOps,
        RandomnessOps, SelfOps, SendOps, UpgradeOps,
    };
    pub use super::{Block, BlockId, BlockRegistry, BlockStat, CallResult, Kernel, SyscallHandler};
    pub use crate::gas::{Gas, GasTimer, PriceList};
    pub use ambassador::Delegate;
    pub use cid::Cid;
    pub use fvm_shared::address::Address;
    pub use fvm_shared::clock::ChainEpoch;
    pub use fvm_shared::crypto::signature::{
        SignatureType, SECP_PUB_LEN, SECP_SIG_LEN, SECP_SIG_MESSAGE_HASH_SIZE,
    };
    pub use fvm_shared::econ::TokenAmount;
    pub use fvm_shared::error::ExitCode;
    pub use fvm_shared::randomness::RANDOMNESS_LENGTH;
    pub use fvm_shared::sys::out::network::NetworkContext;
    pub use fvm_shared::sys::out::vm::MessageContext;
    pub use fvm_shared::sys::SendFlags;
    pub use fvm_shared::version::NetworkVersion;
    pub use fvm_shared::{ActorID, MethodNum};
    pub use multihash::Multihash;
}

use prelude::*;
