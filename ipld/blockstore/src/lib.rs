// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

#[cfg(feature = "resolve")]
/// This module is used for resolving Cids and Ipld recursively. This is generally only needed
/// for testing because links should generally not be collapsed to generate a singular data
/// structure, or this would lead to ambiguity of the data.
pub mod resolve;
#[cfg(feature = "tracking")]
mod tracking;

mod memory;

#[cfg(feature = "tracking")]
pub use self::tracking::{BSStats, TrackingBlockStore};

pub use memory::MemoryBlockstore;

use blockstore::Blockstore;
use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};
use encoding::{de::DeserializeOwned, from_slice, ser::Serialize, to_vec};
use std::error::Error as StdError;

// TODO move this to a multicodec crate.
pub const DAG_CBOR: u64 = 0x71;

/// Wrapper for database to handle inserting and retrieving ipld data with Cids
// TODO: rename to ActorStore. And likely move.
pub trait BlockStore {
    /// Get bytes from block store by Cid.
    fn get_bytes(&self, cid: &Cid) -> Result<Option<Vec<u8>>, Box<dyn StdError>>;

    /// Get typed object from block store by Cid.
    fn get<T>(&self, cid: &Cid) -> Result<Option<T>, Box<dyn StdError>>
    where
        T: DeserializeOwned,
    {
        match self.get_bytes(cid)? {
            Some(bz) => Ok(Some(from_slice(&bz)?)),
            None => Ok(None),
        }
    }

    /// Put an object in the block store and return the Cid identifier.
    fn put<S>(&self, obj: &S, code: Code) -> Result<Cid, Box<dyn StdError>>
    where
        S: Serialize,
    {
        let bytes = to_vec(obj)?;
        self.put_raw(&bytes, code)
    }

    /// Put raw bytes in the block store and return the Cid identifier.
    fn put_raw(&self, bytes: &[u8], code: Code) -> Result<Cid, Box<dyn StdError>>;

    /// Batch put cbor objects into blockstore and returns vector of Cids
    fn bulk_put<'a, S, V>(&self, values: V, code: Code) -> Result<Vec<Cid>, Box<dyn StdError>>
    where
        S: Serialize + 'a,
        V: IntoIterator<Item = &'a S>,
    {
        values
            .into_iter()
            .map(|value| self.put(value, code))
            .collect()
    }
}

impl<T: Blockstore> BlockStore for T {
    fn get_bytes(&self, cid: &Cid) -> Result<Option<Vec<u8>>, Box<dyn StdError>> {
        Ok(self.get(cid)?)
    }

    fn put_raw(&self, bytes: &[u8], code: Code) -> Result<Cid, Box<dyn StdError>> {
        let digest = code.digest(bytes);
        let k = Cid::new_v1(DAG_CBOR, digest);
        self.put(&k, bytes)?;
        Ok(k)
    }
}
