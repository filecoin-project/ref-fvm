use blockstore::Block;
use cid::multihash::Code;
use cid::Cid;
use fvm_shared::error::ExitCode;
use fvm_shared::error::ExitCode::{ErrIllegalState, ErrSerialization};
use std::convert::TryFrom;

/// A blockstore suitable for use within actors.
pub struct ActorBlockstore;

/// Implements a blockstore delegating to IPLD syscalls.
impl blockstore::Blockstore for ActorBlockstore {
    type Error = ExitCode;

    fn get(&self, cid: &Cid) -> Result<Option<Vec<u8>>, Self::Error> {
        // If this fails, the _CID_ is invalid. I.e., we have a bug.
        crate::ipld::get(cid).map(Some).map_err(|_e| {
            // TODO log error; use .inspect_err() to log (unstable) and or() to return the exit code.
            // "get failed with {:?} on CID '{}'", c, cid
            ErrIllegalState
        })
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error> {
        let code = Code::try_from(k.hash().code()).map_err(|_e| {
            // TODO log error; use .inspect_err() to log (unstable) and or() to return the exit code.
            // e.to_string()
            ErrSerialization
        })?;
        let k2 = self.put(code, &Block::new(k.codec(), block))?;
        if k != &k2 {
            // TODO log error
            // "put block with cid {} but has cid {}", k, k2
            Err(ErrSerialization)
        } else {
            Ok(())
        }
    }

    fn put<D>(&self, code: Code, block: &Block<D>) -> Result<Cid, Self::Error>
    where
        D: AsRef<[u8]>,
    {
        // TODO: Don't hard-code the size. Unfortunately, there's no good way to get it from the
        //  codec at the moment.
        const SIZE: u32 = 32;
        crate::ipld::put(code.into(), SIZE, block.codec, block.data.as_ref()).map_err(|_e| {
            // TODO log error; use .inspect_err() to log (unstable) and or() to return the exit code.
            // "put failed with {:?}"
            ErrIllegalState
        })
    }
}
