use blockstore::{Block, Blockstore};
use cid::{multihash::Code, Cid};
use encoding::{de::DeserializeOwned, from_slice, ser::Serialize, to_vec};

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Wrapper for database to handle inserting and retrieving ipld data with Cids
pub trait CborStore: Blockstore + Sized {
    /// Get typed object from block store by Cid.
    fn get_cbor<T>(&self, cid: &Cid) -> Result<Option<T>, Error>
    where
        T: DeserializeOwned,
    {
        match self.get(cid)? {
            Some(bz) => Ok(Some(from_slice(&bz)?)),
            None => Ok(None),
        }
    }

    /// Put an object in the block store and return the Cid identifier.
    fn put_cbor<S>(&self, obj: &S, code: Code) -> Result<Cid, Error>
    where
        S: Serialize,
    {
        let bytes = to_vec(obj)?;
        Ok(self.put(
            code,
            &Block {
                codec: DAG_CBOR,
                data: &bytes,
            },
        )?)
    }
}

impl<T: Blockstore> CborStore for T {}
