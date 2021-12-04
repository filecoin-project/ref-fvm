use super::*;

use anyhow::Result;
use blockstore::Blockstore;
use cid::Cid;
use fvm_shared::ActorID;
use std::convert::TryFrom;

use super::blocks::{Block, BlockRegistry};

/// Tracks data accessed and modified during the execution of a message.
///
/// TODO writes probably ought to be scoped by invocation container.
pub struct DefaultKernel<B> {
    /// Tracks block data and organizes it through index handles so it can be
    /// referred to.
    ///
    /// This does not yet reason about reachability.
    blocks: BlockRegistry,
    store: B,
    /// Current state root of an actor.
    /// TODO This probably doesn't belong here.
    root: Cid,
}

impl<B> DefaultKernel<B> {
    pub fn new(bs: B, root: Cid) -> Self {
        Self {
            root,
            blocks: BlockRegistry::new(),
            store: bs,
        }
    }
}

impl<B> DefaultKernel<B> where B: Blockstore {}

impl<B> ActorOps for DefaultKernel<B>
where
    B: Blockstore,
{
    fn root(&self) -> &Cid {
        &self.root
    }

    fn set_root(&mut self, new: Cid) -> Result<()> {
        self.root = new;
        Ok(())
    }
}

impl<B> BlockOps for DefaultKernel<B>
where
    B: Blockstore,
{
    fn block_open(&mut self, cid: &Cid) -> Result<BlockId, BlockError> {
        let data = self
            .blockstore
            .get(cid)
            .map_err(|e| BlockError::Internal(e.into()))?
            .ok_or_else(|| BlockError::MissingState(Box::new(*cid)))?;

        let block = Block::new(cid.codec(), data);
        self.blocks.put(block)
    }

    fn block_create(&mut self, codec: u64, data: &[u8]) -> Result<BlockId, BlockError> {
        self.blocks.put(Block::new(codec, data))
    }

    fn block_link(&mut self, id: BlockId, hash_fun: u64, hash_len: u32) -> Result<Cid, BlockError> {
        use multihash::MultihashDigest;
        let block = self.blocks.get(id)?;
        let code =
            multihash::Code::try_from(hash_fun)
                .ok()
                .ok_or(BlockError::InvalidMultihashSpec {
                    code: hash_fun,
                    length: hash_len,
                })?;

        let hash = code.digest(&block.data());
        if u32::from(hash.size()) < hash_len {
            return Err(BlockError::InvalidMultihashSpec {
                code: hash_fun,
                length: hash_len,
            });
        }
        let k = Cid::new_v1(block.codec, hash.truncate(hash_len as u8));
        // TODO: for now, we _put_ the block here. In the future, we should put it into a write
        // cache, then flush it later.
        self.store.put(&k, block.data())?;
        Ok(k)
    }

    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<u32, BlockError> {
        let data = &self.blocks.get(id)?.data;
        Ok(if offset as usize >= data.len() {
            0
        } else {
            let len = buf.len().min(data.len());
            buf.copy_from_slice(&data[offset as usize..][..len]);
            len as u32
        })
    }

    fn block_stat(&self, id: BlockId) -> Result<BlockStat, BlockError> {
        self.blocks.get(id).map(|b| BlockStat {
            codec: b.codec(),
            size: b.size(),
        })
    }
}

impl<B> InvocationOps for DefaultKernel<B>
where
    B: Blockstore,
{
    fn method_number(&self) -> MethodId {
        // TODO
        0
    }

    fn method_params(&self) -> BlockId {
        // TODO
        0
    }

    fn caller(&self) -> ActorID {
        // TODO
        1
    }

    fn receiver(&self) -> ActorID {
        // TODO
        0
    }

    fn value_received(&self) -> u128 {
        0
    }
}
