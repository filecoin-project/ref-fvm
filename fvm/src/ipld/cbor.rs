// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::Context;
use cid::Cid;
use fvm_shared::error::ErrorNumber;

use super::{LinkVisitor, Result};

use crate::kernel::ClassifyResult;
use crate::syscall_error;

/// Given a CBOR encoded Buffer, returns a tuple of:
/// the type of the CBOR object along with extra
/// elements we expect to read. More info on this can be found in
/// Appendix C. of RFC 7049 which defines the CBOR specification.
/// This was implemented because the CBOR library we use does not expose low
/// methods like this, requiring us to deserialize the whole CBOR payload, which
/// is unnecessary and quite inefficient for our usecase here.
fn cbor_read_header_buf(br: &mut &[u8]) -> Result<(u8, u64)> {
    #[inline(always)]
    pub fn read_fixed<const N: usize>(r: &mut &[u8]) -> Result<[u8; N]> {
        if r.len() < N {
            return Err(syscall_error!(Serialization; "invalid cbor header").into());
        }

        let mut buf = [0; N];
        buf.copy_from_slice(&r[..N]);
        *r = &r[N..];
        Ok(buf)
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
        _ => return Err(syscall_error!(Serialization; "invalid cbor header").into()),
    };
    Ok((maj, val))
}

/// Walk a DagCBOR IPLD block, visiting each CID discovered.
pub(super) fn scan_for_reachable_links(visitor: &mut LinkVisitor, mut buf: &[u8]) -> Result<()> {
    let mut remaining: u64 = 1;
    while remaining > 0 {
        remaining -= 1;
        visitor.charge_gas(visitor.price_list.ipld_cbor_scan_per_field)?;
        let (maj, extra) = cbor_read_header_buf(&mut buf)?;
        match maj {
            // MajUnsignedInt, MajNegativeInt, MajOther
            0 | 1 | 7 => {}
            // MajByteString, MajTextString
            2 | 3 => {
                if extra > buf.len() as u64 {
                    return Err(
                        syscall_error!(Serialization; "unexpected end of cbor stream").into(),
                    );
                }
                buf = &buf[extra as usize..];
            }
            // MajTag
            6 => {
                // Check if the tag refers to a CID, otherwise continue.
                if extra != 42 {
                    // can't overflow as we subtracted 1 from this variable at the top of the
                    // loop.
                    remaining += 1;
                    continue;
                }
                visitor.charge_gas(visitor.price_list.ipld_cbor_scan_per_cid)?;
                let (maj, extra) = cbor_read_header_buf(&mut buf)?;
                // The actual CID is expected to be a byte string
                if maj != 2 {
                    return Err(
                        syscall_error!(Serialization; "expected cbor type byte string in input")
                            .into(),
                    );
                }
                if extra > buf.len() as u64 {
                    return Err(
                        syscall_error!(Serialization; "unexpected end of cbor stream").into(),
                    );
                }
                if extra < 1 || buf.first() != Some(&0u8) {
                    return Err(
                        syscall_error!(Serialization; "DagCBOR CID does not start with a 0x byte")
                            .into(),
                    );
                }

                // Read the CID and validate it. The CID type itself validates the CID structure
                // and that the digest is less than 64 bytes.
                let mut cid_buf;
                (cid_buf, buf) = buf[1..].split_at(extra as usize - 1);
                let cid = Cid::read_bytes(&mut cid_buf)
                    .map_err(|e| syscall_error!(Serialization; "invalid cid: {e}"))?;
                if !cid_buf.is_empty() {
                    return Err(
                        syscall_error!(Serialization; "cid has {} trailing bytes", cid_buf.len())
                            .into(),
                    );
                }

                // Then visit it.
                visitor.visit_cid(&cid)?;
            }
            // MajArray
            4 => {
                // remaining += extra;
                remaining = remaining
                    .checked_add(extra)
                    .context("cbor field count overflow")
                    .or_error(ErrorNumber::Serialization)?;
            }
            // MajMap
            5 => {
                // remaining += 2 * extra;
                remaining = extra
                    .checked_mul(2)
                    .and_then(|v| v.checked_add(remaining))
                    .context("cbor field count overflow")
                    .or_error(ErrorNumber::Serialization)?;
            }
            8.. => unreachable!("bug in cbor_read_header_buf"),
        }
    }
    if !buf.is_empty() {
        return Err(
            syscall_error!(Serialization; "{} trailing bytes in CBOR block", buf.len()).into(),
        );
    }
    Ok(())
}
