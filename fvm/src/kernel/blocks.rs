use std::convert::TryInto;
use std::rc::Rc;

use fvm_ipld_encoding::DAG_CBOR;
use thiserror::Error;

use super::{ExecutionError, SyscallError};
use crate::syscall_error;

#[derive(Default)]
pub struct BlockRegistry {
    blocks: Vec<Block>,
}

/// Blocks in the block registry are addressed by an ordinal, starting from 1 (`FIRST_ID`).
/// The zero value is reserved to mean "no data", such as when actor invocations
/// receive or return no data.
pub type BlockId = u32;

const FIRST_ID: BlockId = 1;
const MAX_BLOCKS: u32 = i32::MAX as u32; // TODO: Limit

#[derive(Copy, Clone)]
pub struct BlockStat {
    pub codec: u64,
    pub size: u32,
}

#[derive(Debug, Clone)]
pub struct Block {
    codec: u64,
    // Unfortunately, we usually start with a vector/boxed buffer. If we used Rc<[u8]>, we'd have to
    // copy the bytes. So we accept some indirection for reliable performance.
    #[allow(clippy::redundant_allocation)]
    data: Rc<Box<[u8]>>,
}

impl Block {
    pub fn new(codec: u64, data: impl Into<Box<[u8]>>) -> Self {
        // This requires an extra allocation (ew) but no extra copy on send.
        // The extra allocation is basically nothing.
        Self {
            codec,
            data: Rc::new(data.into()),
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
pub enum BlockPutError {
    #[error("too many blocks have been written")]
    TooManyBlocks,
    #[error("invalid or forbidden ipld codec")]
    InvalidCodec(u64),
}

impl From<BlockPutError> for super::SyscallError {
    fn from(e: BlockPutError) -> Self {
        match e {
            BlockPutError::TooManyBlocks => syscall_error!(LimitExceeded; "{}", e),
            BlockPutError::InvalidCodec(_) => syscall_error!(IllegalCodec; "{}", e),
        }
    }
}

impl From<BlockPutError> for ExecutionError {
    fn from(e: BlockPutError) -> Self {
        ExecutionError::Syscall(e.into())
    }
}

#[derive(Error, Debug)]
#[error("block handle {0} does not exist, or is illegal")]
pub struct InvalidHandleError(BlockId);

impl From<InvalidHandleError> for SyscallError {
    fn from(e: InvalidHandleError) -> Self {
        syscall_error!(InvalidHandle; "{}", e)
    }
}

impl From<InvalidHandleError> for ExecutionError {
    fn from(e: InvalidHandleError) -> Self {
        ExecutionError::Syscall(e.into())
    }
}

impl BlockRegistry {
    pub(crate) fn new() -> Self {
        Self { blocks: Vec::new() }
    }
}

impl BlockRegistry {
    /// Adds a new block to the registry, and returns a handle to refer to it.
    pub fn put(&mut self, block: Block) -> Result<BlockId, BlockPutError> {
        if self.is_full() {
            return Err(BlockPutError::TooManyBlocks);
        }
        if block.codec != DAG_CBOR {
            return Err(BlockPutError::InvalidCodec(block.codec));
        }

        let id = FIRST_ID + self.blocks.len() as u32;
        self.blocks.push(block);
        Ok(id)
    }

    /// Gets the block associated with a block handle.
    pub fn get(&self, id: BlockId) -> Result<&Block, InvalidHandleError> {
        if id < FIRST_ID {
            return Err(InvalidHandleError(id));
        }
        id.try_into()
            .ok()
            .and_then(|idx: usize| self.blocks.get(idx - FIRST_ID as usize))
            .ok_or(InvalidHandleError(id))
    }

    /// Returns the size & codec of the specified block.
    pub fn stat(&self, id: BlockId) -> Result<BlockStat, InvalidHandleError> {
        if id < FIRST_ID {
            return Err(InvalidHandleError(id));
        }
        id.try_into()
            .ok()
            .and_then(|idx: usize| self.blocks.get(idx - FIRST_ID as usize))
            .ok_or(InvalidHandleError(id))
            .map(|b| BlockStat {
                codec: b.codec(),
                size: b.size(),
            })
    }

    pub fn is_full(&self) -> bool {
        self.blocks.len() as u32 == MAX_BLOCKS
    }
}
