use std::rc::Rc;

use anyhow::Result;
use cid::multihash::MultihashDigest;
use cid::CidGeneric;

pub mod tracking;

mod memory;
pub use memory::MemoryBlockstore;

mod block;
pub use block::Block;

/// The default size for multihashes is currently set to `64` as it's the value the `multihash` is
/// using is its default code table. Once a custom code table is used, it could be reduced to `32`.
const DEFAULT_MULTIHASH_ALLOC_SIZE: usize = 64;

/// An IPLD blockstore suitable for injection into the FVM.
///
/// The cgo blockstore adapter implements this trait.
///
pub trait Blockstore<const S: usize = DEFAULT_MULTIHASH_ALLOC_SIZE> {
    /// The Multihash code table to use.
    type CodeTable: MultihashDigest<S>;

    /// Gets the block from the blockstore.
    fn get(&self, k: &CidGeneric<S>) -> Result<Option<Vec<u8>>>;

    /// Put a block with a pre-computed cid.
    ///
    /// If you don't yet know the CID, use put. Some blockstores will re-compute the CID internally
    /// even if you provide it.
    ///
    /// If you _do_ already know the CID, use this method as some blockstores _won't_ recompute it.
    fn put_keyed(&self, k: &CidGeneric<S>, block: &[u8]) -> Result<()>;

    /// Checks if the blockstore has the specified block.
    fn has(&self, k: &CidGeneric<S>) -> Result<bool> {
        Ok(self.get(k)?.is_some())
    }

    /// Puts the block into the blockstore, computing the hash with the specified multicodec.
    ///
    /// By default, this defers to put.
    fn put<D>(&self, mh_code: Self::CodeTable, block: &Block<D>) -> Result<CidGeneric<S>>
    where
        Self: Sized,
        D: AsRef<[u8]>,
    {
        let k = block.cid(mh_code);
        self.put_keyed(&k, block.as_ref())?;
        Ok(k)
    }

    /// Bulk put blocks into the blockstore.
    ///
    ///
    /// ```rust
    /// use multihash::Code::Blake2b256;
    /// use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore, Block};
    ///
    /// let bs = MemoryBlockstore::default();
    /// let blocks = vec![Block::new(0x55, vec![0, 1, 2])];
    /// bs.put_many(blocks.iter().map(|b| (Blake2b256, b.into()))).unwrap();
    /// ```
    fn put_many<D, I>(&self, blocks: I) -> Result<()>
    where
        Self: Sized,
        D: AsRef<[u8]>,
        I: IntoIterator<Item = (Self::CodeTable, Block<D>)>,
    {
        self.put_many_keyed(blocks.into_iter().map(|(mc, b)| (b.cid(mc), b)))?;
        Ok(())
    }

    /// Bulk-put pre-keyed blocks into the blockstore.
    ///
    /// By default, this defers to put_keyed.
    fn put_many_keyed<D, I>(&self, blocks: I) -> Result<()>
    where
        Self: Sized,
        D: AsRef<[u8]>,
        I: IntoIterator<Item = (CidGeneric<S>, D)>,
    {
        for (c, b) in blocks {
            self.put_keyed(&c, b.as_ref())?
        }
        Ok(())
    }
}

pub trait Buffered<const S: usize>: Blockstore<S> {
    fn flush(&self, root: &CidGeneric<S>) -> Result<()>;
}

impl<BS, const S: usize> Blockstore<S> for &BS
where
    BS: Blockstore<S>,
{
    type CodeTable = BS::CodeTable;

    fn get(&self, k: &CidGeneric<S>) -> Result<Option<Vec<u8>>> {
        (*self).get(k)
    }

    fn put_keyed(&self, k: &CidGeneric<S>, block: &[u8]) -> Result<()> {
        (*self).put_keyed(k, block)
    }

    fn has(&self, k: &CidGeneric<S>) -> Result<bool> {
        (*self).has(k)
    }

    fn put<D>(&self, mh_code: Self::CodeTable, block: &Block<D>) -> Result<CidGeneric<S>>
    where
        Self: Sized,
        D: AsRef<[u8]>,
    {
        (*self).put(mh_code, block)
    }

    fn put_many<D, I>(&self, blocks: I) -> Result<()>
    where
        Self: Sized,
        D: AsRef<[u8]>,
        I: IntoIterator<Item = (Self::CodeTable, Block<D>)>,
    {
        (*self).put_many(blocks)
    }

    fn put_many_keyed<D, I>(&self, blocks: I) -> Result<()>
    where
        Self: Sized,
        D: AsRef<[u8]>,
        I: IntoIterator<Item = (CidGeneric<S>, D)>,
    {
        (*self).put_many_keyed(blocks)
    }
}

impl<BS, const S: usize> Blockstore<S> for Rc<BS>
where
    BS: Blockstore<S>,
{
    type CodeTable = BS::CodeTable;

    fn get(&self, k: &CidGeneric<S>) -> Result<Option<Vec<u8>>> {
        (**self).get(k)
    }

    fn put_keyed(&self, k: &CidGeneric<S>, block: &[u8]) -> Result<()> {
        (**self).put_keyed(k, block)
    }

    fn has(&self, k: &CidGeneric<S>) -> Result<bool> {
        (**self).has(k)
    }

    fn put<D>(&self, mh_code: Self::CodeTable, block: &Block<D>) -> Result<CidGeneric<S>>
    where
        Self: Sized,
        D: AsRef<[u8]>,
    {
        (**self).put(mh_code, block)
    }

    fn put_many<D, I>(&self, blocks: I) -> Result<()>
    where
        Self: Sized,
        D: AsRef<[u8]>,
        I: IntoIterator<Item = (Self::CodeTable, Block<D>)>,
    {
        (**self).put_many(blocks)
    }

    fn put_many_keyed<D, I>(&self, blocks: I) -> Result<()>
    where
        Self: Sized,
        D: AsRef<[u8]>,
        I: IntoIterator<Item = (CidGeneric<S>, D)>,
    {
        (**self).put_many_keyed(blocks)
    }
}
