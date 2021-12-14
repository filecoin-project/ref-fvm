// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

#![cfg(feature = "tracking")]

use super::BlockStore;
use cid::{multihash::Code, Cid};
use std::cell::RefCell;
use std::error::Error as StdError;

/// Stats for a [TrackingBlockStore] this indicates the amount of read and written data
/// to the wrapped store.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BSStats {
    /// Number of reads
    pub r: usize,
    /// Number of writes
    pub w: usize,
    /// Bytes Read
    pub br: usize,
    /// Bytes Written
    pub bw: usize,
}

/// Wrapper around `BlockStore` to tracking reads and writes for verification.
/// This struct should only be used for testing.
#[derive(Debug)]
pub struct TrackingBlockStore<BS> {
    base: BS,
    pub stats: RefCell<BSStats>,
}

impl<BS> TrackingBlockStore<BS>
where
    BS: BlockStore,
{
    pub fn new(base: BS) -> Self {
        Self {
            base,
            stats: Default::default(),
        }
    }
}

impl<BS> BlockStore for TrackingBlockStore<BS>
where
    BS: BlockStore,
{
    fn get_bytes(&self, cid: &Cid) -> Result<Option<Vec<u8>>, Box<dyn StdError>> {
        self.stats.borrow_mut().r += 1;
        let bytes = self.base.get_bytes(cid)?;
        if let Some(bytes) = &bytes {
            self.stats.borrow_mut().br += bytes.len();
        }
        Ok(bytes)
    }

    fn put_raw(&self, bytes: &[u8], code: Code) -> Result<Cid, Box<dyn StdError>> {
        self.stats.borrow_mut().w += 1;
        self.stats.borrow_mut().bw += bytes.len();
        self.base.put_raw(bytes, code)
    }
}

// Ideally we'd have a blanket impl of BlockStore for &T where T is BlockStore. But we already have that for Blockstore -> BlockStore.
//
// We should find a way to deduplicate these traits.
impl<BS> BlockStore for &TrackingBlockStore<BS>
where
    BS: BlockStore,
{
    fn get_bytes(&self, cid: &Cid) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error>> {
        (*self).get_bytes(cid)
    }

    fn put_raw(&self, bytes: &[u8], code: Code) -> Result<Cid, Box<dyn std::error::Error>> {
        (*self).put_raw(bytes, code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cid::multihash::MultihashDigest;

    #[test]
    fn basic_tracking_store() {
        let mem = blockstore::MemoryBlockstore::default();
        let tr_store = TrackingBlockStore::new(&mem);
        assert_eq!(*tr_store.stats.borrow(), BSStats::default());

        type TestType = (u8, String);
        let object: TestType = (8, "test".to_string());
        let obj_bytes_len = encoding::to_vec(&object).unwrap().len();

        tr_store
            .get::<u8>(&Cid::new_v1(crate::DAG_CBOR, Code::Blake2b256.digest(&[0])))
            .unwrap();
        assert_eq!(
            *tr_store.stats.borrow(),
            BSStats {
                r: 1,
                ..Default::default()
            }
        );

        let put_cid = tr_store.put(&object, Code::Blake2b256).unwrap();
        assert_eq!(tr_store.get::<TestType>(&put_cid).unwrap(), Some(object));
        assert_eq!(
            *tr_store.stats.borrow(),
            BSStats {
                r: 2,
                br: obj_bytes_len,
                w: 1,
                bw: obj_bytes_len,
            }
        );
    }
}
