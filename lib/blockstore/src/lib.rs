use cid::Cid;

// TODO: be generic over size without going insane.
// TODO: maybe have a block _reader_?
pub trait Blockstore {
    type Error;

    fn has(&self, k: &Cid) -> Result<bool, Self::Error>;
    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>, Self::Error>;
    fn put(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error>;
    fn delete(&self, k: &Cid) -> Result<(), Self::Error>;
}

mod memory;

pub use memory::MemoryBlockstore;
