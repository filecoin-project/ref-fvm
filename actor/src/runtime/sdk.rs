use cid::{multihash::Code, Cid};
use fvm_sdk::ipld;
use std::error::Error as StdError; // TODO: nostd!

use ipld_blockstore::{BlockStore, DAG_CBOR};

/// A blockstore suitable for use within actors.
pub struct ActorBlockstore;

impl BlockStore for ActorBlockstore {
    fn get_bytes(&self, cid: &Cid) -> Result<Option<Vec<u8>>, Box<dyn StdError>> {
        Ok(Some(ipld::get(cid)))
    }

    fn put_raw(&self, bytes: &[u8], code: Code) -> Result<Cid, Box<dyn StdError>> {
        // TODO: Don't hard-code the size. Unfortunately, there's no good way to get it from the
        // codec at the moment.
        const SIZE: u32 = 32;
        Ok(ipld::put(code.into(), SIZE, DAG_CBOR, bytes))
    }
}
