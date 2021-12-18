// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::Blockstore;
use cid::Cid;

// TODO: figure out where to put this.
const DAG_CBOR: u64 = 0x71;

// NOTE: This code doesn't currently work. It was taken from the ipld/blockstore crate but now lives here because that "blockstore" is really the actor store.
// TODO:
// 1. Finish converting it to a true blockstore.
// 2. Add bulk put methods to the blockstore.

use std::cell::RefCell;
// TODO: replace HashMap with DashMap like in forest?
use std::{collections::HashMap, error::Error as StdError};

// TODO: This is going to live in the kernel so it should be a Blockstore, not an ActorStore.

/// Wrapper around `Blockstore` to limit and have control over when values are written.
/// This type is not threadsafe and can only be used in synchronous contexts.
#[derive(Debug)]
pub struct BufferedBlockstore<BS> {
    base: BS,
    write: RefCell<HashMap<Cid, Vec<u8>>>,
}

impl<BS> BufferedBlockstore<BS>
where
    BS: Blockstore,
{
    pub fn new(base: BS) -> Self {
        Self {
            base,
            write: Default::default(),
        }
    }

    /// Flushes the buffered cache based on the root node.
    /// This will recursively traverse the cache and write all data connected by links to this
    /// root Cid.
    pub fn flush(&self, root: &Cid) -> Result<(), Box<dyn StdError + '_>> {
        let mut buffer = Vec::new();
        let mut s = self.write.borrow_mut();
        copy_rec(&self.base, &s, *root, &mut buffer)?;

        self.base.put_many_keyed(buffer)?;
        *s = Default::default();

        Ok(())
    }
}

/// Copies the IPLD DAG under `root` from the cache to the base store.
fn copy_rec<'a, BS>(
    base: &BS,
    cache: &'a HashMap<Cid, Vec<u8>>,
    root: Cid,
    buffer: &mut Vec<(Cid, &'a [u8])>,
) -> Result<(), Box<dyn StdError>>
where
    BS: Blockstore,
{
    // TODO: Make this non-recursive.
    // Skip identity and Filecoin commitment Cids
    if root.codec() != DAG_CBOR {
        return Ok(());
    }

    let block = &*cache
        .get(&root)
        .ok_or_else(|| format!("Invalid link ({}) in flushing buffered store", root))?;

    use libipld::cbor::DagCborCodec;
    use libipld::{codec::Codec, Ipld};
    let mut references = Vec::new();
    DagCborCodec.references::<Ipld, _>(block, &mut references)?;

    for link in &references {
        if link.codec() != DAG_CBOR {
            continue;
        }

        // DB reads are expensive. So we check if it exists in the cache.
        // If it doesnt exist in the DB, which is likely, we proceed with using the cache.
        if !cache.contains_key(link) {
            continue;
        }

        // Recursively find more links under the links we're iterating over.
        copy_rec(base, cache, *link, buffer)?;
    }

    buffer.push((root, block));

    Ok(())
}

impl<BS> Blockstore for BufferedBlockstore<BS>
where
    BS: Blockstore,
{
    type Error = BS::Error;

    fn get(&self, cid: &Cid) -> Result<Option<Vec<u8>>, Self::Error> {
        if let Some(data) = self.write.borrow().get(cid) {
            Ok(Some(data.clone()))
        } else {
            self.base.get(cid)
        }
    }

    fn put_keyed(&self, cid: &Cid, buf: &[u8]) -> Result<(), Self::Error> {
        self.write.borrow_mut().insert(*cid, Vec::from(buf));
        Ok(())
    }

    fn has(&self, k: &Cid) -> Result<bool, Self::Error> {
        if self.write.borrow().contains_key(k) {
            Ok(true)
        } else {
            self.base.has(k)
        }
    }

    fn put_many_keyed<D, I>(&self, blocks: I) -> Result<(), Self::Error>
    where
        Self: Sized,
        D: AsRef<[u8]>,
        I: IntoIterator<Item = (Cid, D)>,
    {
        self.write
            .borrow_mut()
            .extend(blocks.into_iter().map(|(k, v)| (k, v.as_ref().into())));
        Ok(())
    }
}

#[cfg(test)]
#[cfg(disabled)]
mod tests {
    use super::*;
    use cid::multihash::{Code, MultihashDigest};
    use forest_ipld::{ipld, Ipld};
    use fvm_shared::commcid::commitment_to_cid;

    const RAW: u64 = 0x55;

    #[test]
    fn basic_buffered_store() {
        let mem = db::MemoryBlockstore::default();
        let mut buf_store = BufferedBlockstore::new(&mem);

        let cid = buf_store.put(&8, Code::Blake2b256).unwrap();
        assert_eq!(mem.get::<u8>(&cid).unwrap(), None);
        assert_eq!(buf_store.get::<u8>(&cid).unwrap(), Some(8));

        buf_store.flush(&cid).unwrap();
        assert_eq!(buf_store.get::<u8>(&cid).unwrap(), Some(8));
        assert_eq!(mem.get::<u8>(&cid).unwrap(), Some(8));
        assert!(buf_store.write.get(&cid).is_none());
    }

    #[test]
    fn buffered_store_with_links() {
        let mem = db::MemoryBlockstore::default();
        let mut buf_store = BufferedBlockstore::new(&mem);
        let str_val = "value";
        let value = 8u8;
        let arr_cid = buf_store.put(&(str_val, value), Code::Blake2b256).unwrap();
        let identity_cid = Cid::new_v1(RAW, Code::Identity.digest(&[0u8]));

        // Create map to insert into store
        let sealed_comm_cid = commitment_to_cid(
            cid::FIL_COMMITMENT_SEALED,
            cid::POSEIDON_BLS12_381_A1_FC1,
            &[7u8; 32],
        )
        .unwrap();
        let unsealed_comm_cid = commitment_to_cid(
            cid::FIL_COMMITMENT_UNSEALED,
            cid::SHA2_256_TRUNC254_PADDED,
            &[5u8; 32],
        )
        .unwrap();
        let map = ipld!({
            "array": Link(arr_cid),
            "sealed": Link(sealed_comm_cid),
            "unsealed": Link(unsealed_comm_cid),
            "identity": Link(identity_cid),
            "value": str_val,
        });
        let map_cid = buf_store.put(&map, Code::Blake2b256).unwrap();

        let root_cid = buf_store.put(&(map_cid, 1u8), Code::Blake2b256).unwrap();

        // Make sure a block not connected to the root does not get written
        let unconnected = buf_store.put(&27u8, Code::Blake2b256).unwrap();

        assert_eq!(mem.get::<Ipld>(&map_cid).unwrap(), None);
        assert_eq!(mem.get::<Ipld>(&root_cid).unwrap(), None);
        assert_eq!(mem.get::<(String, u8)>(&arr_cid).unwrap(), None);
        assert_eq!(buf_store.get::<u8>(&unconnected).unwrap(), Some(27u8));

        // Flush and assert changes
        buf_store.flush(&root_cid).unwrap();
        assert_eq!(
            mem.get::<(String, u8)>(&arr_cid).unwrap(),
            Some((str_val.to_owned(), value))
        );
        assert_eq!(mem.get::<Ipld>(&map_cid).unwrap(), Some(map));
        assert_eq!(
            mem.get::<Ipld>(&root_cid).unwrap(),
            Some(ipld!([Link(map_cid), 1]))
        );
        assert_eq!(buf_store.get::<u8>(&identity_cid).unwrap(), None);
        assert_eq!(buf_store.get::<Ipld>(&unsealed_comm_cid).unwrap(), None);
        assert_eq!(buf_store.get::<Ipld>(&sealed_comm_cid).unwrap(), None);
        assert_eq!(mem.get::<u8>(&unconnected).unwrap(), None);
        assert_eq!(buf_store.get::<u8>(&unconnected).unwrap(), None);
    }
}
