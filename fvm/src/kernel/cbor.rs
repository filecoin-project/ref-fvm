// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::Context;
use cid::Cid;
use fvm_ipld_encoding::DAG_CBOR;
use fvm_shared::error::ErrorNumber;
use num_traits::Zero;

use super::{ExecutionError, Result};

use crate::gas::{Gas, GasTimer, GasTracker, PriceList};
use crate::kernel::{ClassifyResult, ALLOWED_CODECS, IGNORED_CODECS};
use crate::syscall_error;
use std::io::Read;

// TODO: Deduplicate
const BLAKE2B_256: u64 = 0xb220;

/// Given a CBOR encoded Buffer, returns a tuple of:
/// the type of the CBOR object along with extra
/// elements we expect to read. More info on this can be found in
/// Appendix C. of RFC 7049 which defines the CBOR specification.
/// This was implemented because the CBOR library we use does not expose low
/// methods like this, requiring us to deserialize the whole CBOR payload, which
/// is unnecessary and quite inefficient for our usecase here.
fn cbor_read_header_buf<B: Read>(br: &mut B) -> Result<(u8, u64)> {
    #[inline(always)]
    pub fn read_fixed<const N: usize>(r: &mut impl Read) -> Result<[u8; N]> {
        let mut buf = [0; N];
        r.read_exact(&mut buf)
            .map(|_| buf)
            .map_err(|_| syscall_error!(Serialization; "invalid cbor header").into())
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

/// Given a CBOR serialized IPLD buffer, read through all of it and return all "reachable" links.
/// - This function will recursively scan "inline" blocks (identity-hashed CIDs) for recursive
///   links, but won't return inline CIDs directly.
/// - This function will ignore valid Filecoin sector CIDs.
/// - This function will reject blocks that link to blocks with unsupported codecs.
pub(super) fn scan_for_reachable_links(
    buf: &[u8],
    price_list: &PriceList,
    gas_tracker: &GasTracker,
) -> Result<Vec<Cid>> {
    fn inner(
        mut buf: &[u8],
        price_list: &PriceList,
        gas_available: &mut Gas,
        out: &mut Vec<Cid>,
    ) -> Result<()> {
        let mut remaining: u64 = 1;
        while remaining > 0 {
            remaining -= 1;
            if *gas_available < price_list.ipld_cbor_scan_per_field {
                *gas_available = Gas::zero();
                return Err(ExecutionError::OutOfGas);
            }
            *gas_available -= price_list.ipld_cbor_scan_per_field;
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
                    if *gas_available < price_list.ipld_cbor_scan_per_cid {
                        *gas_available = Gas::zero();
                        return Err(ExecutionError::OutOfGas);
                    }
                    *gas_available -= price_list.ipld_cbor_scan_per_cid;
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

                    // Read the CID and validate it. The CID type itself validates that.
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

                    // Then handle it...

                    let codec = cid.codec();

                    if IGNORED_CODECS.contains(&codec) {
                        // NOTE: We don't check multihash codecs here and allow arbitrary hash
                        // digests (assuming the digest is <= 64 bytes).
                        continue;
                    }

                    if !ALLOWED_CODECS.contains(&codec) {
                        // NOTE: We could get away without doing this here _except_ for
                        // identity-hash CIDs. Because, unfortunately, those _don't_ go through the
                        // `ipld::block_create` API.

                        // The error is NotFound because the child is statically "unfindable"
                        // because blocks with this CID cannot exist.
                        return Err(
                                syscall_error!(NotFound; "block links to CID with forbidden codec {codec}")
                                    .into(),
                            );
                    }

                    if cid.hash().code() == fvm_shared::IDENTITY_HASH {
                        if cid.codec() == DAG_CBOR {
                            // TODO: Test max recursion depth. Each level should take 6-7 bytes
                            // leaving at most 11 (likely less) recursive calls (max of a 64
                            // byte digest). We need to make sure this isn't going to be a
                            // problem, or rewrite this to be non-recursive.
                            inner(cid.hash().digest(), price_list, gas_available, out)?;
                        }
                        continue;
                    }

                    if cid.hash().code() != BLAKE2B_256 || cid.hash().size() != 32 {
                        return Err(
                                syscall_error!(NotFound; "block links to CID with forbidden multihash type (code: {}, len: {})", cid.hash().code(), cid.hash().size())
                                    .into(),
                            );
                    }

                    // TODO: Charge a memory retention fee here? Or bundle that into the CID charge
                    // above?
                    out.push(cid);
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
                8.. => {
                    // This case is statically impossible unless `cbor_read_header_buf` makes a mistake.
                    return Err(
                        syscall_error!(Serialization; "invalid cbor tag exceeds 3 bits: {maj}")
                            .into(),
                    );
                }
            }
        }
        if !buf.is_empty() {
            return Err(
                syscall_error!(Serialization; "{} trailing bytes in CBOR block", buf.len()).into(),
            );
        }
        Ok(())
    }
    let start = GasTimer::start();
    let gas_available = gas_tracker.gas_available();
    let mut gas_remaining = gas_available;
    let mut out = Vec::new();
    let ret = inner(buf, price_list, &mut gas_remaining, &mut out);
    let t = gas_tracker.charge_gas("ScanCborLinks", gas_available - gas_remaining)?;
    t.stop_with(start);
    ret.map(|_| out)
}
