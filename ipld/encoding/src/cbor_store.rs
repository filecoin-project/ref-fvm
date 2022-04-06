use cid::{multihash, Cid};
use fvm_ipld_blockstore::{Block, Blockstore};
use serde::{de, ser};

use crate::DAG_CBOR;

#[derive(thiserror::Error, Debug)]
pub enum Error<BS: Blockstore> {
    #[error("blockstore: {0}")]
    Blockstore(BS::Error),
    #[error("encoding: {0}")]
    Encoding(#[from] crate::errors::Error),
}

/// Wrapper for database to handle inserting and retrieving ipld data with Cids
pub trait CborStore: Blockstore + Sized {
    /// Get typed object from block store by Cid.
    fn get_cbor<T>(&self, cid: &Cid) -> Result<Option<T>, Error<Self>>
    where
        T: de::DeserializeOwned,
    {
        match self.get(cid).map_err(Error::Blockstore)? {
            Some(bz) => {
                let res = crate::from_slice(&bz)?;
                Ok(Some(res))
            }
            None => Ok(None),
        }
    }

    /// Put an object in the block store and return the Cid identifier.
    fn put_cbor<S>(&self, obj: &S, code: multihash::Code) -> Result<Cid, Error<Self>>
    where
        S: ser::Serialize,
    {
        let bytes = crate::to_vec(obj)?;
        self.put(
            code,
            &Block {
                codec: DAG_CBOR,
                data: &bytes,
            },
        )
        .map_err(Error::Blockstore)
    }
}

impl<T: Blockstore> CborStore for T {}
