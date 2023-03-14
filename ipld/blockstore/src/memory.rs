// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;

use anyhow::Result;
use cid::Cid;
use multihash::MultihashDigest;

use super::Blockstore;

#[derive(Debug, Clone)]
pub struct MemoryBlockstore<C = multihash::Code> {
    blocks: RefCell<HashMap<Cid, Vec<u8>>>,
    _marker: PhantomData<fn() -> C>,
}

impl Default for MemoryBlockstore<multihash::Code> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C> MemoryBlockstore<C> {
    pub fn new() -> Self {
        Self {
            blocks: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<C> Blockstore for MemoryBlockstore<C>
where
    C: MultihashDigest<64>,
    anyhow::Error: From<C::Error>,
{
    fn has(&self, k: &Cid) -> Result<bool> {
        Ok(self.blocks.borrow().contains_key(k))
    }

    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>> {
        Ok(self.blocks.borrow().get(k).cloned())
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<()> {
        self.blocks.borrow_mut().insert(*k, block.into());
        Ok(())
    }

    fn put(&self, mh_code: u64, block: &dyn crate::Block) -> Result<Cid> {
        let mhcode = C::try_from(mh_code)?;
        let data = block.data();
        let codec = block.codec();

        let mh = mhcode.digest(data);
        let k = Cid::new_v1(codec, mh);
        self.put_keyed(&k, data)?;
        Ok(k)
    }
}

#[test]
fn basic_test() {
    let bs = MemoryBlockstore::default();
    bs.put(multihash::Code::Blake2b256.into(), &(0x55, b"foobar"))
        .unwrap();
}
