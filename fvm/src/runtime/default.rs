use blockstore::Blockstore;
use cid::Cid;
use std::collections::{hash_map::Entry, HashMap};
use std::convert::{TryFrom, TryInto};
use std::rc::Rc;

use super::*;

pub struct DefaultRuntime<B> {
    blocks: HashMap<Cid, BlockState>,
    block_data: BlockRegistry<B>,
    root: Cid,
    config: Config,
}

struct BlockRegistry<B> {
    blocks: Vec<Block>,
    blockstore: B,
}

#[derive(Clone)]
struct Block {
    codec: u64,
    data: Rc<[u8]>,
}

enum BlockState {
    Open { id: u32, dirty: bool },
    Reachable,
}

impl<B> DefaultRuntime<B> {
    pub fn new(config: Config, bs: B, root: Cid) -> Self {
        Self {
            config,
            root,
            block_data: BlockRegistry::new(bs),
            blocks: HashMap::new(),
        }
    }
}

impl<B> BlockRegistry<B> {
    fn new(bs: B) -> Self {
        Self {
            blocks: Vec::new(),
            blockstore: bs,
        }
    }
}

impl<B> BlockRegistry<B>
where
    B: Blockstore,
{
    fn put(&mut self, block: Block) -> Result<BlockId, Error> {
        // TODO: limit the code types we allow.
        let id: u32 = self
            .blocks
            .len()
            .try_into()
            .map_err(|_| Error::TooManyBlocks)?;
        self.blocks.push(block);
        Ok(id)
    }

    fn get(&self, id: BlockId) -> Result<&Block, Error> {
        id.try_into()
            .ok()
            .and_then(|idx: usize| self.blocks.get(idx))
            .ok_or(Error::InvalidHandle)
    }

    fn load(&mut self, cid: &Cid) -> Result<BlockId, Error> {
        self.put(Block {
            codec: cid.codec(),
            data: Rc::from(
                self.blockstore
                    .get(cid)
                    .map_err(|e| Error::Internal(e.into()))?
                    .ok_or(Error::Unreachable)?,
            ),
        })
    }
}

impl<B> DefaultRuntime<B> where B: Blockstore {}

impl<B> IpldRuntime for DefaultRuntime<B>
where
    B: Blockstore,
{
    fn root(&self) -> &Cid {
        &self.root
    }

    fn set_root(&mut self, new: Cid) -> Result<(), Error> {
        if !self.blocks.contains_key(&new) {
            return Err(Error::Unreachable);
        }
        self.root = new;
        Ok(())
    }

    fn block_open(&mut self, cid: &Cid) -> Result<BlockId, Error> {
        // TODO Mark children as reachable.
        match self.blocks.entry(*cid) {
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

    fn block_create(&mut self, codec: u64, data: &[u8]) -> Result<BlockId, Error> {
        // TODO Check that children are reachable.
        self.block_data.put(Block {
            codec,
            data: Rc::from(data),
        })
    }

    fn block_read(&self, id: BlockId, offset: u32, buf: &mut [u8]) -> Result<u32, Error> {
        let data = &self.block_data.get(id)?.data;
        Ok(if offset as usize >= data.len() {
            0
        } else {
            let len = buf.len().min(data.len());
            buf.copy_from_slice(&data[offset as usize..][..len]);
            len as u32
        })
    }

    fn block_stat(&self, id: BlockId) -> Result<BlockStat, Error> {
        self.block_data.get(id).map(|b| BlockStat {
            codec: b.codec,
            size: b.data.len() as u32,
        })
    }

    fn block_cid(&mut self, id: BlockId, hash_fun: u64, hash_len: u32) -> Result<Cid, Error> {
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
            .blocks
            .entry(cid)
            .or_insert(BlockState::Open { id, dirty: true })
        {
            *state = BlockState::Open { id, dirty: true };
        }
        Ok(cid)
    }
}

impl<B> Runtime for DefaultRuntime<B>
where
    B: Blockstore,
{
    fn config(&self) -> &Config {
        &self.config
    }
}
impl<B> InvocationRuntime for DefaultRuntime<B>
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
