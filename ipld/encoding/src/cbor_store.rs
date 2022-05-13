use cid::CidGeneric;
use fvm_ipld_blockstore::{Block, Blockstore};
use serde::{de, ser};

use crate::DAG_CBOR;

/// Wrapper for database to handle inserting and retrieving ipld data with Cids
pub trait CborStore<const S: usize>: Blockstore<S> + Sized {
    /// Get typed object from block store by Cid.
    fn get_cbor<T>(&self, cid: &CidGeneric<S>) -> anyhow::Result<Option<T>>
    where
        T: de::DeserializeOwned,
    {
        match self.get(cid)? {
            Some(bz) => {
                let res = crate::from_slice(&bz)?;
                Ok(Some(res))
            }
            None => Ok(None),
        }
    }

    /// Put an object in the block store and return the Cid identifier.
    fn put_cbor<Ser>(&self, obj: &Ser, code: Self::CodeTable) -> anyhow::Result<CidGeneric<S>>
    where
        Ser: ser::Serialize,
    {
        let bytes = crate::to_vec(obj)?;
        self.put(
            code,
            &Block {
                codec: DAG_CBOR,
                data: &bytes,
            },
        )
    }
}

impl<const S: usize, T: Blockstore<S>> CborStore<S> for T {}
