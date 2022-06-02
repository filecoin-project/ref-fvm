// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek};

use anyhow::{anyhow, Result};
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use cid::Cid;
use fvm_ipld_blockstore::{Blockstore, Buffered};
use fvm_ipld_encoding::DAG_CBOR;
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
    /// root Cid.
    fn flush(&self, root: &Cid) -> Result<()> {
        let mut buffer = Vec::new();
        let mut s = self.write.borrow_mut();
        copy_rec(&s, *root, &mut buffer)?;

        self.base.put_many_keyed(buffer)?;
        *s = Default::default();

        Ok(())
    }
}

/// Given a CBOR encoded Buffer, returns a tuple of:
/// the type of the CBOR object along with extra
/// elements we expect to read. More info on this can be found in
/// Appendix C. of RFC 7049 which defines the CBOR specification.
/// This was implemented because the CBOR library we use does not expose low
/// methods like this, requiring us to deserialize the whole CBOR payload, which
/// is unnecessary and quite inefficient for our usecase here.
fn cbor_read_header_buf<B: Read>(br: &mut B, scratch: &mut [u8]) -> anyhow::Result<(u8, usize)> {
    let first = br.read_u8()?;
    let maj = (first & 0xe0) >> 5;
    let low = first & 0x1f;

    if low < 24 {
        Ok((maj, low as usize))
    } else if low == 24 {
        let val = br.read_u8()?;
        if val < 24 {
            return Err(anyhow!(
                "cbor input was not canonical (lval 24 with value < 24)"
            ));
        }
        Ok((maj, val as usize))
    } else if low == 25 {
        br.read_exact(&mut scratch[..2])?;
        let val = BigEndian::read_u16(&scratch[..2]);
        if val <= u8::MAX as u16 {
            return Err(anyhow!(
                "cbor input was not canonical (lval 25 with value <= MaxUint8)"
            ));
        }
        Ok((maj, val as usize))
    } else if low == 26 {
        br.read_exact(&mut scratch[..4])?;
        let val = BigEndian::read_u32(&scratch[..4]);
        if val <= u16::MAX as u32 {
            return Err(anyhow!(
                "cbor input was not canonical (lval 26 with value <= MaxUint16)"
            ));
        }
        Ok((maj, val as usize))
    } else if low == 27 {
        br.read_exact(&mut scratch[..8])?;
        let val = BigEndian::read_u64(&scratch[..8]);
        if val <= u32::MAX as u64 {
            return Err(anyhow!(
                "cbor input was not canonical (lval 27 with value <= MaxUint32)"
            ));
        }
        Ok((maj, val as usize))
    } else {
        Err(anyhow!("invalid header cbor_read_header_buf"))
    }
}

/// Given a CBOR serialized IPLD buffer, read through all of it and return all the Links.
/// This function is useful because it is quite a bit more fast than doing this recursively on a
/// deserialized IPLD object.
fn scan_for_links<B: Read + Seek, F>(buf: &mut B, mut callback: F) -> Result<()>
where
    F: FnMut(Cid) -> anyhow::Result<()>,
{
    let mut scratch: [u8; 100] = [0; 100];
    let mut remaining = 1;
    while remaining > 0 {
        let (maj, extra) = cbor_read_header_buf(buf, &mut scratch)?;
        match maj {
            // MajUnsignedInt, MajNegativeInt, MajOther
            0 | 1 | 7 => {}
            // MajByteString, MajTextString
            2 | 3 => {
                buf.seek(std::io::SeekFrom::Current(extra as i64))?;
            }
            // MajTag
            6 => {
                // Check if the tag refers to a CID
                if extra == 42 {
                    let (maj, extra) = cbor_read_header_buf(buf, &mut scratch)?;
                    // The actual CID is expected to be a byte string
                    if maj != 2 {
                        return Err(anyhow!("expected cbor type byte string in input"));
                    }
                    if extra > 100 {
                        return Err(anyhow!("string in cbor input too long"));
                    }
                    buf.read_exact(&mut scratch[..extra])?;
                    let c = Cid::try_from(&scratch[1..extra])?;
                    callback(c)?;
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
            _ => {
                return Err(anyhow!("unhandled cbor type: {}", maj));
            }
        }
        remaining -= 1;
    }
    Ok(())
}

/// Copies the IPLD DAG under `root` from the cache to the base store.
fn copy_rec<'a>(
    cache: &'a HashMap<Cid, Vec<u8>>,
    root: Cid,
    buffer: &mut Vec<(Cid, &'a [u8])>,
) -> Result<()> {
    const DAG_RAW: u64 = 0x55;
    const BLAKE2B_256: u64 = 0xb220;
    const BLAKE2B_LEN: u8 = 32;
    const IDENTITY: u64 = 0x0;

    // Differences from lotus (vm.Copy):
    // 1. We assume that if we don't have a block in our buffer, it must already be in the client
    //    and don't check. This should only happen if the client is missing state.
    // 2. We always write-back new blocks, even if the client already has them. We haven't noticed a
    //    perf impact.

    // TODO(M2): Make this not cbor specific.
    match (root.codec(), root.hash().code(), root.hash().size()) {
        // Allow non-truncated blake2b-256 raw/cbor (code/state)
        (DAG_RAW | DAG_CBOR, BLAKE2B_256, BLAKE2B_LEN) => (),
        // Ignore raw identity cids (fake code cids)
        (DAG_RAW, IDENTITY, _) => return Ok(()),
        // Copy links from cbor identity cids.
        // We shouldn't be creating these at the moment, but lotus' vm.Copy supports them.
        (DAG_CBOR, IDENTITY, _) => {
            return scan_for_links(&mut Cursor::new(root.hash().digest()), |link| {
                copy_rec(cache, link, buffer)
            })
        }
        // Ignore commitments (not even going to check the hash function.
        (FIL_COMMITMENT_UNSEALED | FIL_COMMITMENT_SEALED, _, _) => return Ok(()),
        // Fail on anything else. We usually want to continue on error, but there's really no going
        // back from here.
        (codec, hash, length) => {
            return Err(anyhow!(
                "cid {root} has unexpected codec ({codec}), hash ({hash}), or length ({length})"
            ))
        }
    }

    // If we don't have the block, we assume it's already in the datastore.
    //
    // The alternative would be to check if it's in the datastore, but that's likely even more
    // expensive. And there wouldn't be much we could do at that point but abort the block.
    let block = match cache.get(&root) {
        Some(blk) => blk,
        None => return Ok(()),
    };

    // At the moment, we only expect dag-cbor and raw.
    // In M2, we'll need to copy explicitly.
    if root.codec() == DAG_CBOR {
        // TODO(M2): Make this non-recursive.
        scan_for_links(&mut Cursor::new(block), |link| {
            copy_rec(cache, link, buffer)
        })?;
    }

    // Finally, push the block. We do this _last_ so that we always include write before parents.
    buffer.push((root, block));

    Ok(())
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

    const RAW: u64 = 0x55;

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
        assert!(buf_store.write.borrow().get(&cid).is_none());
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
        let identity_cid = Cid::new_v1(RAW, Multihash::wrap(IDENTITY_HASH, &[0]).unwrap());

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
        assert_eq!(buf_store.get_cbor::<u8>(&unconnected).unwrap(), None);
    }
}
