// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::rc::Rc;

use anyhow::Result;
use cid::Cid;

pub mod tracking;

mod memory;
pub use memory::MemoryBlockstore;

pub trait Block {
    fn codec(&self) -> u64;
    fn data(&self) -> &[u8];
    fn len(&self) -> usize {
        self.data().len()
    }
}

impl<T: Block> Block for &T {
    fn codec(&self) -> u64 {
        (**self).codec()
    }
    fn data(&self) -> &[u8] {
        (**self).data()
    }
}

impl<T: Block> Block for Box<T> {
    fn codec(&self) -> u64 {
        (**self).codec()
    }
    fn data(&self) -> &[u8] {
        (**self).data()
    }
}

impl<T: Block> Block for Rc<T> {
    fn codec(&self) -> u64 {
        (**self).codec()
    }
    fn data(&self) -> &[u8] {
        (**self).data()
    }
}

impl<D: AsRef<[u8]>> Block for (u64, D) {
    fn codec(&self) -> u64 {
        self.0
    }

    fn data(&self) -> &[u8] {
        self.1.as_ref()
    }
}

/// An IPLD blockstore suitable for injection into the FVM.
///
/// The cgo blockstore adapter implements this trait.
pub trait Blockstore {
    /// Gets the block from the blockstore.
    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>>;

    /// Put a block with a pre-computed cid.
    ///
    /// If you don't yet know the CID, use put. Some blockstores will re-compute the CID internally
    /// even if you provide it.
    ///
    /// If you _do_ already know the CID, use this method as some blockstores _won't_ recompute it.
    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<()>;

    /// Checks if the blockstore has the specified block.
    fn has(&self, k: &Cid) -> Result<bool> {
        Ok(self.get(k)?.is_some())
    }

    /// Puts the block into the blockstore, computing the hash with the specified multicodec.
    fn put(&self, mh_code: u64, block: &dyn Block) -> Result<Cid>;

    /// Bulk put blocks into the blockstore.
    ///
    ///
    /// ```rust
    /// use multihash::Code::Blake2b256;
    /// use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore, Block};
    ///
    /// let bs = MemoryBlockstore::default();
    /// let blocks = vec![(0x55, vec![0, 1, 2])];
    /// bs.put_many(blocks.iter().map(|b| (Blake2b256.into(), b))).unwrap();
    /// ```
    fn put_many<B, I>(&self, blocks: I) -> Result<()>
    where
        Self: Sized,
        B: Block,
        I: IntoIterator<Item = (u64, B)>,
    {
        for (code, block) in blocks {
            self.put(code, &block)?;
        }
        Ok(())
    }

    /// Bulk-put pre-keyed blocks into the blockstore.
    ///
    /// By default, this defers to put_keyed.
    fn put_many_keyed<D, I>(&self, blocks: I) -> Result<()>
    where
        Self: Sized,
        D: AsRef<[u8]>,
        I: IntoIterator<Item = (Cid, D)>,
    {
        for (c, b) in blocks {
            self.put_keyed(&c, b.as_ref())?
        }
        Ok(())
    }
}

pub trait Buffered: Blockstore {
    fn flush(&self, root: &Cid) -> Result<()>;
}

impl<BS> Blockstore for &BS
where
    BS: Blockstore,
{
    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>> {
        (*self).get(k)
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<()> {
        (*self).put_keyed(k, block)
    }

    fn has(&self, k: &Cid) -> Result<bool> {
        (*self).has(k)
    }

    fn put(&self, mh_code: u64, block: &dyn Block) -> Result<Cid> {
        (*self).put(mh_code, block)
    }

    fn put_many<B, I>(&self, blocks: I) -> Result<()>
    where
        Self: Sized,
        B: Block,
        I: IntoIterator<Item = (u64, B)>,
    {
        (*self).put_many(blocks)
    }

    fn put_many_keyed<D, I>(&self, blocks: I) -> Result<()>
    where
        Self: Sized,
        D: AsRef<[u8]>,
        I: IntoIterator<Item = (Cid, D)>,
    {
        (*self).put_many_keyed(blocks)
    }
}

impl<BS> Blockstore for Rc<BS>
where
    BS: Blockstore,
{
    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>> {
        (**self).get(k)
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<()> {
        (**self).put_keyed(k, block)
    }

    fn has(&self, k: &Cid) -> Result<bool> {
        (**self).has(k)
    }

    fn put(&self, mh_code: u64, block: &dyn Block) -> Result<Cid> {
        (**self).put(mh_code, block)
    }

    fn put_many<B, I>(&self, blocks: I) -> Result<()>
    where
        Self: Sized,
        B: Block,
        I: IntoIterator<Item = (u64, B)>,
    {
        (**self).put_many(blocks)
    }

    fn put_many_keyed<D, I>(&self, blocks: I) -> Result<()>
    where
        Self: Sized,
        D: AsRef<[u8]>,
        I: IntoIterator<Item = (Cid, D)>,
    {
        (**self).put_many_keyed(blocks)
    }
}
