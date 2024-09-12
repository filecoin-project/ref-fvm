// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Read;

use anyhow::{anyhow, Result};
use cid::Cid;
use fvm_ipld_blockstore::{Blockstore, Buffered};
use fvm_ipld_encoding::{CBOR, DAG_CBOR, IPLD_RAW};
use fvm_shared::commcid::{FIL_COMMITMENT_SEALED, FIL_COMMITMENT_UNSEALED};

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

    pub fn into_inner(self) -> BS {
        self.base
    }
}

impl<BS> Buffered for BufferedBlockstore<BS>
where
    BS: Blockstore,
{
    /// Flushes the buffered cache based on the root node.
    /// This will recursively traverse the cache and write all data connected by links to this
    /// root Cid, moving the reachable blocks from the write buffer to the backing store.
    fn flush(&self, root: &Cid) -> Result<()> {
        self.base
            .put_many_keyed(take_reachable(&mut self.write.borrow_mut(), root)?)
    }
}

/// Given a CBOR encoded Buffer, returns a tuple of:
/// the type of the CBOR object along with extra
/// elements we expect to read. More info on this can be found in
/// Appendix C. of RFC 7049 which defines the CBOR specification.
/// This was implemented because the CBOR library we use does not expose low
/// methods like this, requiring us to deserialize the whole CBOR payload, which
/// is unnecessary and quite inefficient for our usecase here.
fn cbor_read_header_buf<B: Read>(br: &mut B) -> anyhow::Result<(u8, u64)> {
    #[inline(always)]
    pub fn read_fixed<const N: usize>(r: &mut impl Read) -> std::io::Result<[u8; N]> {
        let mut buf = [0; N];
        r.read_exact(&mut buf).map(|_| buf)
    }

    let first = read_fixed::<1>(br)?[0];
    let maj = (first & 0xe0) >> 5;
    let low = first & 0x1f;

    let val = match low {
        ..=23 => low.into(),
        24 => read_fixed::<1>(br)?[0].into(),
        25 => u16::from_be_bytes(read_fixed(br)?).into(),
        26 => u32::from_be_bytes(read_fixed(br)?).into(),
        27 => u64::from_be_bytes(read_fixed(br)?),
        _ => return Err(anyhow!("invalid header cbor_read_header_buf")),
    };
    Ok((maj, val))
}

/// Given a CBOR serialized IPLD buffer, read through all of it and return all the Links.
/// This function is useful because it is quite a bit more fast than doing this recursively on a
/// deserialized IPLD object.
fn scan_for_links(mut buf: &[u8], out: &mut Vec<Cid>) -> Result<()> {
    let mut remaining = 1;
    while remaining > 0 {
        let (maj, extra) = cbor_read_header_buf(&mut buf)?;
        match maj {
            // MajUnsignedInt, MajNegativeInt, MajOther
            0 | 1 | 7 => {}
            // MajByteString, MajTextString
            2 | 3 => {
                if extra > buf.len() as u64 {
                    return Err(anyhow!("unexpected end of cbor stream"));
                }
                buf = &buf[extra as usize..];
            }
            // MajTag
            6 => {
                // Check if the tag refers to a CID
                if extra == 42 {
                    let (maj, extra) = cbor_read_header_buf(&mut buf)?;
                    // The actual CID is expected to be a byte string
                    if maj != 2 {
                        return Err(anyhow!("expected cbor type byte string in input"));
                    }
                    if extra > buf.len() as u64 {
                        return Err(anyhow!("unexpected end of cbor stream"));
                    }
                    if buf.first() != Some(&0u8) {
                        return Err(anyhow!("DagCBOR CID does not start with a 0x byte"));
                    }
                    let cid_buf;
                    (cid_buf, buf) = buf.split_at(extra as usize);
                    out.push(Cid::try_from(&cid_buf[1..])?);
                } else {
                    remaining += 1;
                }
            }
            // MajArray
            4 => {
                remaining += extra;
            }
            // MajMap
            5 => {
                remaining += extra * 2;
            }
            8.. => {
                // This case is statically impossible unless `cbor_read_header_buf` makes a mistake.
                return Err(anyhow!("invalid cbor tag exceeds 3 bits: {}", maj));
            }
        }
        remaining -= 1;
    }
    Ok(())
}

/// Moves the IPLD DAG under `root` from the cache to the base store.
fn take_reachable(cache: &mut HashMap<Cid, Vec<u8>>, root: &Cid) -> Result<Vec<(Cid, Vec<u8>)>> {
    const BLAKE2B_256: u64 = 0xb220;
    const BLAKE2B_LEN: u8 = 32;
    const IDENTITY: u64 = 0x0;

    // Differences from lotus (vm.Copy):
    // 1. We assume that if we don't have a block in our buffer, it must already be in the client
    //    and don't check. This should only happen if the client is missing state.
    // 2. We always write-back new blocks, even if the client already has them. We haven't noticed a
    //    perf impact.

    let mut stack = vec![*root];
    let mut result = Vec::new();

    while let Some(k) = stack.pop() {
        // Check the codec.
        match k.codec() {
            // We ignore piece commitment CIDs.
            FIL_COMMITMENT_UNSEALED | FIL_COMMITMENT_SEALED => continue,
            // We allow raw, cbor, and dag cbor.
            IPLD_RAW | DAG_CBOR | CBOR => (),
            // Everything else is rejected.
            codec => return Err(anyhow!("cid {k} has unexpected codec ({codec})")),
        }
        // Check the hash construction.
        match (k.hash().code(), k.hash().size()) {
            // Allow non-truncated blake2b-256 and identity hashes.
            (BLAKE2B_256, BLAKE2B_LEN) | (IDENTITY, _) => (),
            // Reject everything else.
            (hash, length) => {
                return Err(anyhow!(
                    "cid {k} has unexpected multihash (code={hash}, len={length})"
                ))
            }
        }
        if k.hash().code() == IDENTITY {
            if k.codec() == DAG_CBOR {
                scan_for_links(k.hash().digest(), &mut stack)?;
            }
        } else {
            // If we don't have the block, we assume it and it's children are already in the
            // datastore.
            //
            // The alternative would be to check if it's in the datastore, but that's likely even more
            // expensive. And there wouldn't be much we could do at that point but abort the block.
            let Some(block) = cache.remove(&k) else {
                continue;
            };

            // At the moment, only DAG_CBOR can link to other blocks.
            if k.codec() == DAG_CBOR {
                scan_for_links(&block, &mut stack)?;
            }

            // Record the block so we can write it back.
            result.push((k, block));
        };
    }

    Ok(result)
}

impl<BS> Blockstore for BufferedBlockstore<BS>
where
    BS: Blockstore,
{
    fn get(&self, cid: &Cid) -> Result<Option<Vec<u8>>> {
        Ok(if let Some(data) = self.write.borrow().get(cid) {
            Some(data.clone())
        } else {
            self.base.get(cid)?
        })
    }

    fn put_keyed(&self, cid: &Cid, buf: &[u8]) -> Result<()> {
        self.write.borrow_mut().insert(*cid, Vec::from(buf));
        Ok(())
    }

    fn has(&self, k: &Cid) -> Result<bool> {
        if self.write.borrow().contains_key(k) {
            Ok(true)
        } else {
            Ok(self.base.has(k)?)
        }
    }

    fn put_many_keyed<D, I>(&self, blocks: I) -> Result<()>
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
mod tests {
    use cid::multihash::{Code, Multihash};
    use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
    use fvm_ipld_encoding::CborStore;
    use fvm_shared::{commcid, IDENTITY_HASH};
    use serde::{Deserialize, Serialize};

    use super::*;

    #[test]
    fn basic_buffered_store() {
        let mem = MemoryBlockstore::default();
        let buf_store = BufferedBlockstore::new(&mem);

        let cid = buf_store.put_cbor(&8u8, Code::Blake2b256).unwrap();
        assert_eq!(mem.get_cbor::<u8>(&cid).unwrap(), None);
        assert_eq!(buf_store.get_cbor::<u8>(&cid).unwrap(), Some(8));

        buf_store.flush(&cid).unwrap();
        assert_eq!(buf_store.get_cbor::<u8>(&cid).unwrap(), Some(8));
        assert_eq!(mem.get_cbor::<u8>(&cid).unwrap(), Some(8));
    }

    #[test]
    fn buffered_store_with_links() {
        let mem = MemoryBlockstore::default();
        let buf_store = BufferedBlockstore::new(&mem);
        let str_val = String::from("value");
        let value = 8u8;
        let arr_cid = buf_store
            .put_cbor(&(str_val.clone(), value), Code::Blake2b256)
            .unwrap();
        let identity_cid = Cid::new_v1(CBOR, Multihash::wrap(IDENTITY_HASH, &[0]).unwrap());

        // Create map to insert into store
        let sealed_comm_cid = commcid::commitment_to_cid(
            commcid::FIL_COMMITMENT_SEALED,
            commcid::POSEIDON_BLS12_381_A1_FC1,
            &[7u8; 32],
        )
        .unwrap();
        let unsealed_comm_cid = commcid::commitment_to_cid(
            commcid::FIL_COMMITMENT_UNSEALED,
            commcid::SHA2_256_TRUNC254_PADDED,
            &[5u8; 32],
        )
        .unwrap();
        #[derive(Deserialize, Serialize, PartialEq, Eq, Debug)]
        struct TestObject {
            array: Cid,
            sealed: Cid,
            unsealed: Cid,
            identity: Cid,
            value: String,
        }
        let obj = TestObject {
            array: arr_cid,
            sealed: sealed_comm_cid,
            unsealed: unsealed_comm_cid,
            identity: identity_cid,
            value: str_val.clone(),
        };
        let obj_cid = buf_store.put_cbor(&obj, Code::Blake2b256).unwrap();

        let root_cid = buf_store
            .put_cbor(&(obj_cid, 1u8), Code::Blake2b256)
            .unwrap();

        // Make sure a block not connected to the root does not get written
        let unconnected = buf_store.put_cbor(&27u8, Code::Blake2b256).unwrap();

        assert_eq!(mem.get_cbor::<TestObject>(&obj_cid).unwrap(), None);
        assert_eq!(mem.get_cbor::<(Cid, u8)>(&root_cid).unwrap(), None);
        assert_eq!(mem.get_cbor::<(String, u8)>(&arr_cid).unwrap(), None);
        assert_eq!(buf_store.get_cbor::<u8>(&unconnected).unwrap(), Some(27u8));

        // Flush and assert changes
        buf_store.flush(&root_cid).unwrap();
        assert_eq!(
            mem.get_cbor::<(String, u8)>(&arr_cid).unwrap(),
            Some((str_val, value))
        );
        assert_eq!(mem.get_cbor::<TestObject>(&obj_cid).unwrap(), Some(obj));
        assert_eq!(
            mem.get_cbor::<(Cid, u8)>(&root_cid).unwrap(),
            Some((obj_cid, 1)),
        );
        assert_eq!(buf_store.get_cbor::<u8>(&identity_cid).unwrap(), None);
        assert_eq!(buf_store.get(&unsealed_comm_cid).unwrap(), None);
        assert_eq!(buf_store.get(&sealed_comm_cid).unwrap(), None);
        assert_eq!(mem.get_cbor::<u8>(&unconnected).unwrap(), None);
    }
}
