use cid::Cid;

pub struct Blockstore;

impl blockstore::Blockstore for Blockstore {
    type Error = (); // SyscallError;

    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>, Self::Error> {
        // TODO handle syscall errors.
        let vec = crate::ipld::get(k);
        Ok(Some(vec))
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error> {
        // TODO error handling
        crate::ipld::put(k.hash().code(), k.hash().size() as u32, k.codec(), block);
        Ok(())
    }
}
