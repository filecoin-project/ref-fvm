pub use blocks::{BlockError, BlockId, BlockStat};
use cid::Cid;
use fvm_ipld_encoding::RawBytes;
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::econ::TokenAmount;
use fvm_shared::piece::PieceInfo;
use fvm_shared::randomness::{Randomness, RANDOMNESS_LENGTH};
use fvm_shared::sector::{
    AggregateSealVerifyProofAndInfos, RegisteredSealProof, ReplicaUpdateInfo, SealVerifyInfo,
    WindowPoStVerifyInfo,
};
use fvm_shared::version::NetworkVersion;
use fvm_shared::{actor, ActorID, MethodNum};

mod blocks;
pub mod default;

mod error;

pub use error::{ClassifyResult, Context, ExecutionError, Result, SyscallError};

use crate::call_manager::{CallManager, InvocationResult};
use crate::gas::PriceList;
use crate::machine::Machine;

/// The "kernel" implements
pub trait Kernel:
    ActorOps
    + BlockOps
    + CircSupplyOps
    + CryptoOps
    + DebugOps
    + GasOps
    + MessageOps
    + NetworkOps
    + RandomnessOps
    + SelfOps
    + SendOps
    + 'static
{
    /// The [`Kernel`]'s [`CallManager`] is
    type CallManager: CallManager;

    /// Consume the [`Kernel`] and return the underlying [`CallManager`].
    fn into_call_manager(self) -> Self::CallManager
    where
        Self: Sized;

    /// Construct a new [`Kernel`] from the given [`CallManager`].
    ///
    /// - `caller` is the ID of the _immediate_ caller.
    /// - `actor_id` is the ID of _this_ actor.
    /// - `method` is the method that has been invoked.
    /// - `value_received` is value received due to the current call.
    fn new(
        mgr: Self::CallManager,
        caller: ActorID,
        actor_id: ActorID,
        method: MethodNum,
        value_received: TokenAmount,
    ) -> Self
    where
        Self: Sized;
}

/// Network-related operations.
pub trait NetworkOps {
    /// The current network epoch (constant).
    fn network_epoch(&self) -> ChainEpoch;

    /// The current network version (constant).
    fn network_version(&self) -> NetworkVersion;

    /// The current base-fee (constant).
    fn network_base_fee(&self) -> &TokenAmount;
}

/// Accessors to query attributes of the incoming message.
pub trait MessageOps {
    /// The calling actor (constant).
    fn msg_caller(&self) -> ActorID;

    /// The receiving actor (this actor) (constant).
    fn msg_receiver(&self) -> ActorID;

    /// The method number used to invoke this actor (constant).
    fn msg_method_number(&self) -> MethodNum;

    /// The value received from the caller (constant).
    fn msg_value_received(&self) -> TokenAmount;
}

/// The IPLD subset of the kernel.
pub trait BlockOps {
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
    fn block_read(&mut self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<u32>;

    /// Returns the blocks codec & size.
    ///
    /// This method will fail if the block handle is invalid.
    fn block_stat(&mut self, id: BlockId) -> Result<BlockStat>;

    /// Returns a codec and a block as an owned buffer, given an ID.
    ///
    /// This method will fail if the block handle is invalid.
    fn block_get(&mut self, id: BlockId) -> Result<(u64, Vec<u8>)> {
        let stat = self.block_stat(id)?;
        let mut ret = vec![0; stat.size as usize];
        // TODO error handling.
        let read = self.block_read(id, 0, &mut ret)?;
        debug_assert_eq!(stat.size, read, "didn't read expected bytes");
        Ok((stat.codec, ret))
    }

    // TODO: add a way to _flush_ new blocks.
}

/// Actor state access and manipulation.
/// Depends on BlockOps to read and write blocks in the state tree.
pub trait SelfOps: BlockOps {
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
    fn resolve_address(&self, address: &Address) -> Result<Option<ActorID>>;

    /// Look up the code ID at an actor address.
    fn get_actor_code_cid(&self, addr: &Address) -> Result<Option<Cid>>;

    /// Computes an address for a new actor. The returned address is intended to uniquely refer to
    /// the actor even in the event of a chain re-org (whereas an ID-address might refer to a
    /// different actor after messages are re-ordered).
    /// Always an ActorExec address.
    fn new_actor_address(&mut self) -> Result<Address>;

    /// Creates an actor with code `code_cid` and id `actor_id`, with empty state.
    /// May only be called by Init actor.
    fn create_actor(&mut self, code_cid: Cid, actor_id: ActorID) -> Result<()>;

    /// Returns whether the supplied code_cid belongs to a known built-in actor type.
    fn resolve_builtin_actor_type(&self, code_cid: &Cid) -> Option<actor::builtin::Type>;

    /// Returns the CodeCID for the supplied built-in actor type.
    fn get_code_cid_for_type(&self, typ: actor::builtin::Type) -> Result<Cid>;
}

/// Operations to send messages to other actors.
pub trait SendOps {
    fn send(
        &mut self,
        recipient: &Address,
        method: u64,
        params: &RawBytes,
        value: &TokenAmount,
    ) -> Result<InvocationResult>;
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
///
/// TODO this is unsafe; most gas charges should occur as part of syscalls, but
///  some built-in actors currently charge gas explicitly for concrete actions.
///  In the future (Phase 1), this should disappear and be replaced by gas instrumentation
///  at the WASM level.
pub trait GasOps {
    /// GasUsed return the gas used by the transaction so far.
    fn gas_used(&self) -> i64;

    /// ChargeGas charges specified amount of `gas` for execution.
    /// `name` provides information about gas charging point
    fn charge_gas(&mut self, name: &str, compute: i64) -> Result<()>;

    fn price_list(&self) -> &PriceList;
}

/// Cryptographic primitives provided by the kernel.
pub trait CryptoOps {
    /// Verifies that a signature is valid for an address and plaintext.
    fn verify_signature(
        &mut self,
        signature: &Signature,
        signer: &Address,
        plaintext: &[u8],
    ) -> Result<bool>;

    /// Hashes input data using blake2b with 256 bit output.
    fn hash_blake2b(&mut self, data: &[u8]) -> Result<[u8; 32]>;

    /// Computes an unsealed sector CID (CommD) from its constituent piece CIDs (CommPs) and sizes.
    fn compute_unsealed_sector_cid(
        &mut self,
        proof_type: RegisteredSealProof,
        pieces: &[PieceInfo],
    ) -> Result<Cid>;

    /// Verifies a sector seal proof.
    fn verify_seal(&mut self, vi: &SealVerifyInfo) -> Result<bool>;

    /// Verifies a window proof of spacetime.
    fn verify_post(&mut self, verify_info: &WindowPoStVerifyInfo) -> Result<bool>;

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
        &mut self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> Result<Option<ConsensusFault>>;

    /// Verifies a batch of seals. This is a privledged syscall, may _only_ be called by the
    /// power actor during cron.
    ///
    /// Gas: This syscall intentionally _does not_ charge any gas (as said gas would be charged to
    /// cron). Instead, gas is pre-paid by the storage provider on pre-commit.
    fn batch_verify_seals(&mut self, vis: &[SealVerifyInfo]) -> Result<Vec<bool>>;

    /// Verify aggregate seals verifies an aggregated batch of prove-commits.
    fn verify_aggregate_seals(
        &mut self,
        aggregate: &AggregateSealVerifyProofAndInfos,
    ) -> Result<bool>;

    /// Verify replica update verifies a snap deal: an upgrade from a CC sector to a sector with
    /// deals.
    fn verify_replica_update(&mut self, replica: &ReplicaUpdateInfo) -> Result<bool>;
}

/// Randomness queries.
pub trait RandomnessOps {
    /// Randomness returns a (pseudo)random byte array drawing from the latest
    /// ticket chain from a given epoch and incorporating requisite entropy.
    /// This randomness is fork dependant but also biasable because of this.
    fn get_randomness_from_tickets(
        &mut self,
        personalization: DomainSeparationTag,
        rand_epoch: ChainEpoch,
        entropy: &[u8],
    ) -> Result<[u8; RANDOMNESS_LENGTH]>;

    /// Randomness returns a (pseudo)random byte array drawing from the latest
    /// beacon from a given epoch and incorporating requisite entropy.
    /// This randomness is not tied to any fork of the chain, and is unbiasable.
    fn get_randomness_from_beacon(
        &mut self,
        personalization: DomainSeparationTag,
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
}
