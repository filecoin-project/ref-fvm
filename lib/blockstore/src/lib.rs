use cid::Cid;

#[cfg(not(feature = "std"))]
use core2::error::Error;
#[cfg(not(feature = "std"))]
use core2::io::Read;

pub trait BlockReader {
    fn read(&self, buf: &mut [u8], offset: usize);
    fn size(&self) -> usize;
    fn cid(&self) -> &Cid;
}

// TODO: be generic over size without going insane.
pub trait Blockstore {
    type Error: Error;
    type Block: BlockReader;

    fn has(&self, k: &Cid) -> Result<bool, Self::Error>;
    fn get(&self, k: &Cid) -> Result<Option<Self::Block>, Self::Error>;
    fn put(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error>;
    fn delete(&self, k: &Cid) -> Result<(), Self::Error>;
}
