use cid::{multihash, Cid};
use fvm_ipld_encoding::{de, from_slice, ser, to_vec, DAG_CBOR};

use super::{Block, Blockstore};

/// Wrapper for database to handle inserting and retrieving ipld data with Cids
pub trait CborStore: Blockstore + Sized {
    /// Get typed object from block store by Cid.
    fn get_cbor<T>(&self, cid: &Cid) -> anyhow::Result<Option<T>>
    where
        T: de::DeserializeOwned,
    {
        match self.get(cid)? {
            Some(bz) => Ok(Some(from_slice(&bz)?)),
            None => Ok(None),
        }
    }

    /// Put an object in the block store and return the Cid identifier.
    fn put_cbor<S>(&self, obj: &S, code: multihash::Code) -> anyhow::Result<Cid>
    where
        S: ser::Serialize,
    {
        let bytes = to_vec(obj)?;
        self.put(
            code,
            &Block {
                codec: DAG_CBOR,
                data: &bytes,
            },
        )
    }
}

impl<T: Blockstore> CborStore for T {}
