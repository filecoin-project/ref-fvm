use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::Infallible;

use cid::Cid;

use super::Blockstore;

#[derive(Debug, Default, Clone)]
pub struct MemoryBlockstore {
    blocks: RefCell<HashMap<Cid, Vec<u8>>>,
}

impl MemoryBlockstore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Blockstore for MemoryBlockstore {
    type Error = Infallible;

    fn has(&self, k: &Cid) -> Result<bool, Self::Error> {
        Ok(self.blocks.borrow().contains_key(k))
    }

    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.blocks.borrow().get(k).cloned())
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error> {
        self.blocks.borrow_mut().insert(*k, block.into());
        Ok(())
    }
}
