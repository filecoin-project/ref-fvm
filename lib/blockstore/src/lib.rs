use std::rc::Rc;

use cid::Cid;

pub mod buffered;
mod memory;
pub use memory::MemoryBlockstore;

#[cfg(feature = "cgo")]
pub mod cgo;

/// An IPLD blockstore suitable for injection into the FVM.
///
/// The cgo blockstore adapter implements this trait.
pub trait Blockstore {
    /// The concrete error type that the implementation will throw.
    type Error: std::error::Error + Send + Sync + 'static;

    fn has(&self, k: &Cid) -> Result<bool, Self::Error>;
    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>, Self::Error>;
    fn put(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error>;
    fn delete(&self, k: &Cid) -> Result<(), Self::Error>;

    fn put_many<'a, I>(&self, blocks: I) -> Result<(), Self::Error>
    where
        Self: Sized,
        I: IntoIterator<Item = (Cid, &'a [u8])>,
    {
        for (k, b) in blocks {
            self.put(&k, b)?;
        }
        Ok(())
    }
}

impl<B> Blockstore for &B
where
    B: Blockstore,
{
    type Error = B::Error;

    fn has(&self, k: &Cid) -> Result<bool, Self::Error> {
        (*self).has(k)
    }

    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>, Self::Error> {
        (*self).get(k)
    }

    fn put(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error> {
        (*self).put(k, block)
    }

    fn delete(&self, k: &Cid) -> Result<(), Self::Error> {
        (*self).delete(k)
    }

    fn put_many<'a, I>(&self, blocks: I) -> Result<(), Self::Error>
    where
        Self: Sized,
        I: IntoIterator<Item = (Cid, &'a [u8])>,
    {
        (*self).put_many(blocks)
    }
}

impl<B> Blockstore for Rc<B>
where
    B: Blockstore,
{
    type Error = B::Error;

    fn has(&self, k: &Cid) -> Result<bool, Self::Error> {
        (**self).has(k)
    }

    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>, Self::Error> {
        (**self).get(k)
    }

    fn put(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error> {
        (**self).put(k, block)
    }

    fn delete(&self, k: &Cid) -> Result<(), Self::Error> {
        (**self).delete(k)
    }

    fn put_many<'a, I>(&self, blocks: I) -> Result<(), Self::Error>
    where
        Self: Sized,
        I: IntoIterator<Item = (Cid, &'a [u8])>,
    {
        (**self).put_many(blocks)
    }
}
