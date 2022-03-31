use cid::{multihash, Cid};
use serde::{de, ser};

use super::{Block, Blockstore};

// TODO find something to reference.
pub const DAG_CBOR: u64 = 0x71;

/// Wrapper for database to handle inserting and retrieving ipld data with Cids
pub trait CborStore: Blockstore + Sized {
    /// Get typed object from block store by Cid.
    fn get_cbor<T>(&self, cid: &Cid) -> anyhow::Result<Option<T>>
    where
        T: de::DeserializeOwned,
    {
        match self.get(cid)? {
            Some(bz) => {
                let res = serde_ipld_dagcbor::from_slice(&bz)?;
                Ok(Some(res))
            }
            None => Ok(None),
        }
    }

    /// Put an object in the block store and return the Cid identifier.
    fn put_cbor<S>(&self, obj: &S, code: multihash::Code) -> anyhow::Result<Cid>
    where
        S: ser::Serialize,
    {
        let mut bytes = Vec::new();
        obj.serialize(&mut serde_ipld_dagcbor::Serializer::new(&mut bytes))?;
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
