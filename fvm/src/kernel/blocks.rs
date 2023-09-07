use std::collections::HashSet;
// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::rc::Rc;

use cid::Cid;
use fvm_ipld_encoding::ipld_block::IpldBlock;

use super::Result;
use crate::syscall_error;

/// A registry of open blocks (per-kernel). Think "file descriptor" table. At the moment, there's no
/// way to close/remove a block from this table.
#[derive(Default)]
pub struct BlockRegistry {
    blocks: Vec<Block>,
    reachable: HashSet<Cid>,
}

/// Blocks in the block registry are addressed by an ordinal, starting from 1 (`FIRST_ID`).
/// The zero value is reserved to mean "no data", such as when actor invocations
/// receive or return no data.
pub type BlockId = u32;

const FIRST_ID: BlockId = 1;
const MAX_BLOCKS: u32 = i32::MAX as u32; // TODO(M2): Limit

#[derive(Debug, Copy, Clone)]
pub struct BlockStat {
    pub codec: u64,
    pub size: u32,
}

#[derive(Debug, Clone)]
pub struct Block(Rc<BlockInner>);
#[derive(Debug)]
struct BlockInner {
    codec: u64,
    data: Box<[u8]>,
    links: Box<[Cid]>,
}

impl Block {
    pub fn new(codec: u64, data: impl Into<Box<[u8]>>, links: impl Into<Box<[Cid]>>) -> Self {
        // This requires an extra allocation (ew) but no extra copy on send.
        // The extra allocation is basically nothing.
        Self(Rc::new(BlockInner {
            codec,
            data: data.into(),
            links: links.into(),
        }))
    }

    #[inline(always)]
    pub fn codec(&self) -> u64 {
        self.0.codec
    }

    #[inline(always)]
    pub fn links(&self) -> &[Cid] {
        &self.0.links
    }

    #[inline(always)]
    pub fn data(&self) -> &[u8] {
        &self.0.data
    }

    #[inline(always)]
    pub fn size(&self) -> u32 {
        self.0.data.len() as u32
    }

    #[inline(always)]
    pub fn stat(&self) -> BlockStat {
        BlockStat {
            codec: self.codec(),
            size: self.size(),
        }
    }
}

impl From<&Block> for IpldBlock {
    fn from(b: &Block) -> Self {
        IpldBlock {
            codec: b.0.codec,
            data: Vec::from(&*b.0.data),
        }
    }
}

impl BlockRegistry {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

impl BlockRegistry {
    /// Adds a new block to the registry, marking all children as reachable, returning a handle to
    /// refer to it. Use this when adding a block known to be reachable.
    pub fn put_reachable(&mut self, block: Block) -> Result<BlockId> {
        self.put_inner(block, false)
    }

    /// Adds a new block to the registry, checking that all children are currently reachable,
    /// returning a handle to refer to it. Use this when creating a _new_ block.
    //
    //  Returns a `NotFound` error if `block` references any unreachable CIDs.
    pub fn put_check_reachable(&mut self, block: Block) -> Result<BlockId> {
        self.put_inner(block, true)
    }

    /// Mark a cid as reachable. Call this when a new block is linked into the state.
    pub fn mark_reachable(&mut self, k: &Cid) {
        self.reachable.insert(*k);
    }

    /// Check if a block is reachable. Call this before attempting to read the block from the
    /// datastore.
    pub fn is_reachable(&self, k: &Cid) -> bool {
        // NOTE: do not implicitly treat inline blocks (identity-hashed CIDs) as "reachable" as they
        // may contain links to _unreachable_ children.
        self.reachable.contains(k)
    }

    /// Adds a new block to the registry, and returns a handle to refer to it.
    fn put_inner(&mut self, block: Block, check_reachable: bool) -> Result<BlockId> {
        if self.is_full() {
            return Err(syscall_error!(LimitExceeded; "too many blocks").into());
        }

        // We expect the caller to have already charged for gas.
        if check_reachable {
            if let Some(k) = block.links().iter().find(|k| !self.is_reachable(k)) {
                return Err(syscall_error!(NotFound; "cannot put block: {k} not reachable").into());
            }
        } else {
            for k in block.links() {
                self.mark_reachable(k)
            }
        }

        let id = FIRST_ID + self.blocks.len() as u32;
        self.blocks.push(block);
        Ok(id)
    }

    /// Gets the block associated with a block handle.
    pub fn get(&self, id: BlockId) -> Result<&Block> {
        if id < FIRST_ID {
            return Err(syscall_error!(InvalidHandle; "invalid block handle {id}").into());
        }
        id.try_into()
            .ok()
            .and_then(|idx: usize| self.blocks.get(idx - FIRST_ID as usize))
            .ok_or(syscall_error!(InvalidHandle; "invalid block handle {id}").into())
    }

    /// Returns the size & codec of the specified block.
    pub fn stat(&self, id: BlockId) -> Result<BlockStat> {
        if id < FIRST_ID {
            return Err(syscall_error!(InvalidHandle; "invalid block handle {id}").into());
        }
        id.try_into()
            .ok()
            .and_then(|idx: usize| self.blocks.get(idx - FIRST_ID as usize))
            .ok_or(syscall_error!(InvalidHandle; "invalid block handle {id}").into())
            .map(|b| BlockStat {
                codec: b.codec(),
                size: b.size(),
            })
    }

    pub fn is_full(&self) -> bool {
        self.blocks.len() as u32 == MAX_BLOCKS
    }
}
