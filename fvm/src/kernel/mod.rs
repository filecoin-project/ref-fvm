use anyhow::Result;
use cid::Cid;

pub use blocks::{BlockError, BlockId, BlockStat};
use fvm_shared::ActorID;

mod blocks;
pub mod default;

/// TODO likely don't need the Blockstore type parameter since the
/// blockstore will be accessed through the externs.
pub trait Kernel: ActorOps + BlockOps + InvocationOps {}

pub type MethodId = u64;

pub trait InvocationOps {
    fn method_number(&self) -> MethodId;
    fn method_params(&self) -> BlockId;
    fn caller(&self) -> ActorID;
    fn receiver(&self) -> ActorID;
    fn value_received(&self) -> u128;
}

/// The IPLD subset of the runtime.
pub trait BlockOps {
    /// Open a block.
    ///
    /// This method will fail if the requested block isn't reachable.
    fn block_open(&mut self, cid: &Cid) -> Result<BlockId, BlockError>;

    /// Create a new block.
    ///
    /// This method will fail if the block is too large (SPEC_AUDIT), the codec is not allowed
    /// (SPEC_AUDIT), the block references unreachable blocks, or the block contains too many links
    /// (SPEC_AUDIT).
    fn block_create(&mut self, codec: u64, data: &[u8]) -> Result<BlockId, BlockError>;

    /// Computes a CID for a block.
    ///
    /// This is the only way to add a new block to the "reachable" set.
    ///
    /// This method will fail if the block handle is invalid.
    fn block_link(&mut self, id: BlockId, hash_fun: u64, hash_len: u32) -> Result<Cid, BlockError>;

    /// Read data from a block.
    ///
    /// This method will fail if the block handle is invalid.
    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<u32, BlockError>;

    /// Returns the blocks codec & size.
    ///
    /// This method will fail if the block handle is invalid.
    fn block_stat(&self, id: BlockId) -> Result<BlockStat, BlockError>;

    // TODO: add a way to _flush_ new blocks.
}

/// Actor state access and manipulation. Depends on BlockOps to read and write
/// blocks in the state tree.
pub trait ActorOps: BlockOps {
    /// Get the state root.
    fn root(&self) -> &Cid;

    /// Update the state-root.
    ///
    /// This method will fail if the new state-root isn't reachable.
    fn set_root(&mut self, root: Cid) -> anyhow::Result<()>;
}
