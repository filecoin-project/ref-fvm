use cid::Cid;

#[cfg(feature = "buffered")]
pub mod buffered;
mod memory;
pub use memory::MemoryBlockstore;

#[cfg(feature = "cgo")]
pub mod cgo;

/// An IPLD blockstore suitable for injection into the FVM.
///
/// The cgo blockstore adapter implements this trait.
///
// TODO: be generic over size without going insane.
// TODO: maybe have a block _reader_?
pub trait Blockstore {
    /// The concrete error type that the implementation will throw.
    type Error: std::error::Error + Send + 'static;

    fn has(&self, k: &Cid) -> Result<bool, Self::Error>;
    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>, Self::Error>;
    fn put(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error>;
    fn delete(&self, k: &Cid) -> Result<(), Self::Error>;
}
