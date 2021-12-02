use super::*;
use crate::externs::Externs;
use anyhow::Result;
use blockstore::Blockstore;
use cid::Cid;
use fvm_shared::ActorID;
use std::collections::{hash_map::Entry, HashMap};
use std::convert::{TryFrom, TryInto};
use std::rc::Rc;

/// Tracks data accessed and modified during the execution of a message.
///
/// TODO writes probably ought to be scoped by invocation container.
pub struct DefaultKernel<B: Blockstore> {
    /// Tracks the state of blocks that have been brought in scope of
    /// an execution.
    block_state: HashMap<Cid, BlockState>,
    /// Tracks block data and organizes it through index handles so it can be
    /// referred to.
    block_data: BlockRegistry<B>,
    /// Current state root of an actor.
    /// TODO This probably doesn't belong here.
    root: Cid,
}

impl<B> DefaultKernel<B> {
    pub fn new(bs: B, root: Cid) -> Self {
        Self {
            root,
            block_data: BlockRegistry::new(bs),
            block_state: HashMap::new(),
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
        if !self.block_state.contains_key(&new) {
            return Err(Error::Unreachable);
        }
        self.root = new;
        Ok(())
    }
}

impl<B> BlockOps for DefaultKernel<B>
where
    B: Blockstore,
{
    fn block_open(&mut self, cid: &Cid) -> Result<BlockId, BlockError> {
        // TODO Mark children as reachable.
        match self.block_state.entry(*cid) {
            Entry::Occupied(mut entry) => match entry.get_mut() {
                BlockState::Open { id, .. } => {
                    self.block_data.put(self.block_data.get(*id)?.clone())
                }
                state @ BlockState::Reachable => {
                    let id = self.block_data.load(cid)?;
                    *state = BlockState::Open { id, dirty: false };
                    Ok(id)
                }
            },
            Entry::Vacant(entry) => {
                let id = self.block_data.load(cid)?;
                entry.insert(BlockState::Open { id, dirty: false });
                Ok(id)
            }
        }
    }

    fn block_create(&mut self, codec: u64, data: &[u8]) -> Result<BlockId, BlockError> {
        // TODO Check that children are reachable.
        self.block_data.put(Block {
            codec,
            data: Rc::from(data),
        })
    }

    fn block_cid(&mut self, id: BlockId, hash_fun: u64, hash_len: u32) -> Result<Cid, BlockError> {
        // TODO: limit the hash functions/sizes.

        use multihash::MultihashDigest;
        let block = self.block_data.get(id)?;
        let code = multihash::Code::try_from(hash_fun)
            .ok()
            .ok_or(Error::InvalidMultihashSpec)?;

        let hash = code.digest(&block.data);
        if u32::from(hash.size()) < hash_len {
            return Err(Error::InvalidMultihashSpec);
        }
        let cid = Cid::new_v1(block.codec, hash.truncate(hash_len as u8));

        if let state @ BlockState::Reachable = self
            .block_state
            .entry(cid)
            .or_insert(BlockState::Open { id, dirty: true })
        {
            *state = BlockState::Open { id, dirty: true };
        }
        Ok(cid)
    }

    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<u32, BlockError> {
        let data = &self.block_data.get(id)?.data;
        Ok(if offset as usize >= data.len() {
            0
        } else {
            let len = buf.len().min(data.len());
            buf.copy_from_slice(&data[offset as usize..][..len]);
            len as u32
        })
    }

    fn block_stat(&self, id: BlockId) -> Result<BlockStat, BlockError> {
        self.block_data.get(id).map(|b| BlockStat {
            codec: b.codec,
            size: b.data.len() as u32,
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
