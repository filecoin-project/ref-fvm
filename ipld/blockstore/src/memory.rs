use std::cell::RefCell;
use std::collections::HashMap;

use anyhow::Result;
use cid::CidGeneric;

use super::{Blockstore, DEFAULT_MULTIHASH_ALLOC_SIZE};

#[derive(Debug, Default, Clone)]
pub struct MemoryBlockstore<const S: usize = DEFAULT_MULTIHASH_ALLOC_SIZE> {
    blocks: RefCell<HashMap<CidGeneric<S>, Vec<u8>>>,
}

impl<const S: usize> MemoryBlockstore<S> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Blockstore<DEFAULT_MULTIHASH_ALLOC_SIZE> for MemoryBlockstore<DEFAULT_MULTIHASH_ALLOC_SIZE> {
    type CodeTable = multihash::Code;

    fn has(&self, k: &CidGeneric<DEFAULT_MULTIHASH_ALLOC_SIZE>) -> Result<bool> {
        Ok(self.blocks.borrow().contains_key(k))
    }

    fn get(&self, k: &CidGeneric<DEFAULT_MULTIHASH_ALLOC_SIZE>) -> Result<Option<Vec<u8>>> {
        Ok(self.blocks.borrow().get(k).cloned())
    }

    fn put_keyed(&self, k: &CidGeneric<DEFAULT_MULTIHASH_ALLOC_SIZE>, block: &[u8]) -> Result<()> {
        self.blocks.borrow_mut().insert(*k, block.into());
        Ok(())
    }
}
