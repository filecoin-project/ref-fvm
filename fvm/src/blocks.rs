use crate::plumbing::{BlockError, BlockId};
use blockstore::Blockstore;
use cid::Cid;
use std::convert::TryInto;
use std::rc::Rc;

struct BlockRegistry<B: Blockstore> {
    blocks: Vec<Block>,
    blockstore: B,
}

#[derive(Clone)]
struct Block {
    codec: u64,
    data: Rc<[u8]>,
}

#[allow(dead_code)]
enum BlockState {
    Open { id: u32, dirty: bool },
    Reachable,
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
    /// Adds a new block to the registry, and returns a handle to refer to it.
    fn put(&mut self, block: Block) -> Result<BlockId, BlockError> {
        // TODO: limit the code types we allow.
        let id: u32 = self
            .blocks
            .len()
            .try_into()
            .map_err(|_| Error::TooManyBlocks)?;
        self.blocks.push(block);
        Ok(id)
    }

    /// Gets the block associated with a block handle.
    fn get(&self, id: BlockId) -> Result<&Block, BlockError> {
        id.try_into()
            .ok()
            .and_then(|idx: usize| self.blocks.get(idx))
            .ok_or(Error::InvalidHandle)
    }

    /// Loads the block identified by the supplied CID from the blockstore,
    /// and returns a handle to refer to it.
    fn load(&mut self, cid: &Cid) -> Result<BlockId, BlockError> {
        let loaded = self
            .blockstore
            .get(cid)
            .map_err(|e| Error::Internal(e.into()))?
            .ok_or(Error::Unreachable)?;

        let block = Block {
            codec: cid.codec(),
            data: Rc::from(loaded),
        };
        self.put(block)
    }
}
