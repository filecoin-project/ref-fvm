use cid::Cid;
use thiserror::Error;

mod default;
pub use default::*;

#[derive(Copy, Clone)]
pub struct Config {
    pub max_pages: usize,
}

pub type BlockId = u32;

pub struct BlockStat {
    pub codec: u64,
    pub size: u32,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("block is unreachable")]
    Unreachable,
    #[error("too many blocks have been written")]
    TooManyBlocks,
    #[error("block handle does not exist")]
    InvalidHandle,
    #[error("invalid multihash length or code")]
    InvalidMultihashSpec,
    #[error("invalid or forbidden ipld codec")]
    InvalidCodec,
    #[error("internal error: {0}")]
    Internal(#[source] Box<dyn std::error::Error>),
}

/// The IPLD subset of the runtime.
pub trait IpldRuntime {
    /// Get the state root.
    fn root(&self) -> &Cid;

    /// Update the state-root.
    ///
    /// This method will fail if the new state-root isn't reachable.
    fn set_root(&mut self, root: Cid) -> Result<(), Error>;

    /// Open a block.
    ///
    /// This method will fail if the requested block isn't reachable.
    fn block_open(&mut self, cid: &Cid) -> Result<BlockId, Error>;

    /// Create a new block.
    ///
    /// This method will fail if the block is too large (SPEC_AUDIT), the codec is not allowed
    /// (SPEC_AUDIT), the block references unreachable blocks, or the block contains too many links
    /// (SPEC_AUDIT).
    fn block_create(&mut self, codec: u64, data: &[u8]) -> Result<BlockId, Error>;

    /// Computes a CID for a block.
    ///
    /// This is the only way to add a new block to the "reachable" set.
    ///
    /// This method will fail if the block handle is invalid.
    fn block_cid(&mut self, id: BlockId, hash_fun: u64, hash_len: u32) -> Result<Cid, Error>;

    /// Read data from a block.
    ///
    /// This method will fail if the block handle is invalid.
    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<u32, Error>;

    /// Returns the blocks codec & size.
    ///
    /// This method will fail if the block handle is invalid.
    fn block_stat(&self, id: BlockId) -> Result<BlockStat, Error>;

    // TODO: add a way to _flush_ new blocks.
}

type MethodId = u64;
type ActorID = u64;

pub trait InvocationRuntime {
    fn method_number(&self) -> MethodId;
    fn method_params(&self) -> BlockId;
    fn caller(&self) -> ActorID;
    fn receiver(&self) -> ActorID;
    fn value_received(&self) -> u128;
}

pub trait Runtime: IpldRuntime + InvocationRuntime {
    fn config(&self) -> &Config;
}
