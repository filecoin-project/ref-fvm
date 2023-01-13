use std::cell::RefCell;
use std::collections::HashSet;

use anyhow::Result;
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;

#[derive(Debug)]
pub struct TracingBlockStore<BS: Blockstore> {
    pub base: BS,
    pub traced: RefCell<HashSet<Cid>>,
}

impl<BS> TracingBlockStore<BS>
where
    BS: Blockstore,
{
    pub fn new(base: BS) -> Self {
        Self {
            base,
            traced: Default::default(),
        }
    }
}

impl<BS> Blockstore for TracingBlockStore<BS>
where
    BS: Blockstore,
{
    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>> {
        let mut traced = self.traced.borrow_mut();
        traced.insert(*k);
        self.base.get(k)
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<()> {
        let mut traced = self.traced.borrow_mut();
        traced.insert(*k);
        self.base.put_keyed(k, block)
    }
}
