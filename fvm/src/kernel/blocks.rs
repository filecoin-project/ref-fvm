use std::convert::TryInto;

use cid::Cid;
use thiserror::Error;

#[derive(Default)]
pub(crate) struct BlockRegistry {
    blocks: Vec<Block>,
}

/// Blocks in the block registry are addressed by an ordinal, starting from 1 (`FIRST_ID`).
/// The zero value is reserved to mean "no data", such as when actor invocations
/// receive or return no data.
pub type BlockId = u32;

const FIRST_ID: BlockId = 1;

#[derive(Copy, Clone)]
pub struct BlockStat {
    pub codec: u64,
    pub size: u32,
}

#[derive(Clone)]
pub struct Block {
    codec: u64,
    data: Box<[u8]>,
}

impl Block {
    pub fn new(codec: u64, data: impl Into<Box<[u8]>>) -> Self {
        // TODO: check size on the way in?
        Self {
            codec,
            data: data.into(),
        }
    }

    #[inline(always)]
    pub fn codec(&self) -> u64 {
        self.codec
    }

    #[inline(always)]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    #[inline(always)]
    pub fn size(&self) -> u32 {
        self.data.len() as u32
    }

    #[inline(always)]
    pub fn stat(&self) -> BlockStat {
        BlockStat {
            codec: self.codec(),
            size: self.size(),
        }
    }
}

#[derive(Error, Debug)]
pub enum BlockError {
    #[error("block {0} is unreachable")]
    Unreachable(Box<Cid>),
    #[error("too many blocks have been written")]
    TooManyBlocks,
    #[error("block handle {0} does not exist, or is illegal")]
    InvalidHandle(BlockId),
    #[error("invalid multihash length or code")]
    InvalidMultihashSpec { length: u32, code: u64 },
    #[error("invalid or forbidden ipld codec")]
    InvalidCodec(u64),
    #[error("state {0} is missing from the local datastore")]
    MissingState(Box<Cid>), // boxed because CIDs are potentially large.
}

impl BlockRegistry {
    pub(crate) fn new() -> Self {
        Self { blocks: Vec::new() }
    }
}

impl BlockRegistry {
    /// Adds a new block to the registry, and returns a handle to refer to it.
    pub fn put(&mut self, block: Block) -> Result<BlockId, BlockError> {
        // TODO: limit the code types we allow.
        let mut id: u32 = self
            .blocks
            .len()
            .try_into()
            .map_err(|_| BlockError::TooManyBlocks)?;
        id += FIRST_ID;
        self.blocks.push(block);
        Ok(id)
    }

    /// Gets the block associated with a block handle.
    pub fn get(&self, id: BlockId) -> Result<&Block, BlockError> {
        if id < FIRST_ID {
            return Err(BlockError::InvalidHandle(id));
        }
        id.try_into()
            .ok()
            .and_then(|idx: usize| self.blocks.get(idx - FIRST_ID as usize))
            .ok_or(BlockError::InvalidHandle(id))
    }

    /// Returns the size & codec of the specified block.
    pub fn stat(&self, id: BlockId) -> Result<BlockStat, BlockError> {
        if id < FIRST_ID {
            return Err(BlockError::InvalidHandle(id));
        }
        id.try_into()
            .ok()
            .and_then(|idx: usize| self.blocks.get(idx - FIRST_ID as usize))
            .ok_or(BlockError::InvalidHandle(id))
            .map(|b| BlockStat {
                codec: b.codec(),
                size: b.size(),
            })
    }
}
