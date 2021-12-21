use cid::{multihash::Code, Cid};
use fvm_sdk::ipld;
use fvm_shared::error::ExitCode;
use std::convert::TryFrom;

use crate::ActorError;
use blockstore::{Block, Blockstore};

/// A blockstore suitable for use within actors.
pub struct ActorBlockstore;

impl Blockstore for ActorBlockstore {
    type Error = ActorError;

    fn get(&self, cid: &Cid) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(Some(ipld::get(cid)?))
    }

    fn put<D>(&self, code: Code, block: &Block<D>) -> Result<Cid, Self::Error>
    where
        D: AsRef<[u8]>,
    {
        // TODO: Don't hard-code the size. Unfortunately, there's no good way to get it from the
        // codec at the moment.
        const SIZE: u32 = 32;
        Ok(ipld::put(
            code.into(),
            SIZE,
            block.codec,
            block.data.as_ref(),
        )?)
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<(), Self::Error> {
        let k2 = self.put(
            Code::try_from(k.hash().code())
                .map_err(|e| ActorError::new(ExitCode::ErrSerialization, e.to_string()))?,
            &Block::new(k.codec(), block),
        )?;
        if k != &k2 {
            Err(ActorError::new(
                ExitCode::ErrSerialization,
                format!("put block with cid {} but has cid {}", k, k2),
            ))
        } else {
            Ok(())
        }
    }
}
