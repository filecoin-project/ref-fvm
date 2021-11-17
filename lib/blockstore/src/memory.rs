use super::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::Infallible; // TODO: move to ! someday.

#[derive(Default, Clone)]
pub struct MemoryBlockstore {
    // TODO: make the blockstore take self by mut to avoid this?
    // Trying to match forest for now.
    blocks: RefCell<HashMap<Cid, Vec<u8>>>,
}

impl Blockstore for MemoryBlockstore {
    type Error = Infallible;
    fn has(&self, k: &Cid) -> Result<bool, Self::Error> {
        Ok(self.blocks.borrow().contains_key(k))
    }
    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.blocks.borrow().get(k).cloned())
    }
    fn put(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error> {
        self.blocks.borrow_mut().insert(*k, Vec::from(block));
        Ok(())
    }
    fn delete(&self, k: &Cid) -> Result<(), Self::Error> {
        self.blocks.borrow_mut().remove(k);
        Ok(())
    }
}
