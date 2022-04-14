// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::{anyhow, Result};
use byteorder::{BigEndian, ByteOrder, ReadBytesExt};
use cid::Cid;
use fvm_ipld_blockstore::{Blockstore, Buffered};

// TODO: figure out where to put this.
const DAG_CBOR: u64 = 0x71;

// TODO: replace HashMap with DashMap like in forest?
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek};

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
    // TODO: Make this non-recursive.
    // Skip identity and Filecoin commitment Cids
    if root.codec() != DAG_CBOR {
        return Ok(());
    }

    let block = &*cache
        .get(&root)
        .ok_or_else(|| anyhow!("Invalid link ({}) in flushing buffered store", root))?;

    scan_for_links(&mut Cursor::new(block), |link| {
        if link.codec() != DAG_CBOR {
            return Ok(());
        }

        // DB reads are expensive. So we check if it exists in the cache.
        // If it doesnt exist in the DB, which is likely, we proceed with using the cache.
        if !cache.contains_key(&link) {
            return Ok(());
        }

        // Recursively find more links under the links we're iterating over.
        copy_rec(cache, link, buffer)
    })?;

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
