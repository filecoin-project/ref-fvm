use std::convert::TryInto;
use std::rc::Rc;

use cid::Cid;
use thiserror::Error;

#[derive(Default)]
pub(crate) struct BlockRegistry {
    blocks: Vec<Block>,
}

pub type BlockId = u32;

#[derive(Copy, Clone)]
pub struct BlockStat {
    pub codec: u64,
    pub size: u32,
}

#[derive(Clone)]
pub struct Block {
    // TODO rm pub, provide accessors and constructor instead?
    pub(crate) codec: u64,
    pub(crate) data: Rc<[u8]>,
}

impl Block {
    pub fn new(codec: u64, data: impl Into<Rc<[u8]>>) -> Self {
        // TODO: check size on the way in?
        Self {
            codec,
            data: data.into(),
        }
    }

    pub fn codec(&self) -> u64 {
        self.codec
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn size(&self) -> u32 {
        self.data.len() as u32
    }
}

#[derive(Error, Debug)]
pub enum BlockError {
    #[error("block {0} is unreachable")]
    Unreachable(Box<Cid>),
    #[error("too many blocks have been written")]
    TooManyBlocks,
    #[error("block handle {0} does not exist")]
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
        let id: u32 = self
            .blocks
            .len()
            .try_into()
            .map_err(|_| BlockError::TooManyBlocks)?;
        self.blocks.push(block);
        Ok(id)
    }

    /// Gets the block associated with a block handle.
    pub fn get(&self, id: BlockId) -> Result<&Block, BlockError> {
        id.try_into()
            .ok()
            .and_then(|idx: usize| self.blocks.get(idx))
            .ok_or(BlockError::InvalidHandle(id))
    }

    /// Returns the size & codec of the specified block.
    pub fn stat(&self, id: BlockId) -> Result<BlockStat, BlockError> {
        id.try_into()
            .ok()
            .and_then(|idx: usize| self.blocks.get(idx))
            .ok_or(BlockError::InvalidHandle(id))
            .map(|b| BlockStat {
                codec: b.codec(),
                size: b.size(),
            })
    }
}
