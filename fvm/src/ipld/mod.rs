// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::Cid;
use fvm_ipld_encoding::{CBOR, DAG_CBOR, IPLD_RAW};
use fvm_shared::commcid::{FIL_COMMITMENT_SEALED, FIL_COMMITMENT_UNSEALED};
use num_traits::Zero;

use crate::gas::{Gas, GasTimer, GasTracker, PriceList};
use crate::kernel::{ExecutionError, Result};
use crate::syscall_error;

mod cbor;

struct LinkVisitor<'a> {
    pub price_list: &'a PriceList,
    gas_available: Gas,
    gas_remaining: Gas,
    links: Vec<Cid>,
}

/// Codecs allowed by the IPLD subsystem.
pub const ALLOWED_CODECS: &[u64] = &[CBOR, DAG_CBOR, IPLD_RAW];
/// Codecs ignored by the IPLD subsystem.
pub const IGNORED_CODECS: &[u64] = &[FIL_COMMITMENT_UNSEALED, FIL_COMMITMENT_SEALED];

// TODO: Deduplicate
const BLAKE2B_256: u64 = 0xb220;

impl<'a> LinkVisitor<'a> {
    pub fn new(price_list: &'a PriceList, gas_available: Gas) -> Self {
        Self {
            price_list,
            gas_available,
            gas_remaining: gas_available,
            links: Vec::new(),
        }
    }

    pub fn finish(mut self) -> Vec<Cid> {
        self.links.shrink_to_fit();
        self.links
    }

    pub fn gas_used(&self) -> Gas {
        self.gas_available - self.gas_remaining
    }

    #[cold]
    fn out_of_gas(&mut self) -> Result<()> {
        self.gas_remaining = Gas::zero();
        Err(ExecutionError::OutOfGas)
    }

    /// Charge for gas used, returning an error if we run out.
    #[inline(always)]
    pub fn charge_gas(&mut self, gas: Gas) -> Result<()> {
        if self.gas_remaining < gas {
            self.out_of_gas()
        } else {
            self.gas_remaining -= gas;
            Ok(())
        }
    }

    /// Visit a CID, possibly ignoring it or even recursively scanning it.
    /// - This function will recursively scan "inline" blocks (identity-hashed CIDs) for recursive
    ///   links, but won't return inline CIDs directly.
    /// - This function will ignore valid Filecoin sector CIDs.
    /// - This function will reject blocks that link to blocks with unsupported codecs.
    pub fn visit_cid(&mut self, cid: &Cid) -> Result<()> {
        let codec = cid.codec();

        if IGNORED_CODECS.contains(&codec) {
            // NOTE: We don't check multihash codecs here and allow arbitrary hash
            // digests (assuming the digest is <= 64 bytes).
            return Ok(());
        }

        if !ALLOWED_CODECS.contains(&codec) {
            // NOTE: We could get away without doing this here _except_ for
            // identity-hash CIDs. Because, unfortunately, those _don't_ go through the
            // `ipld::block_create` API.

            // The error is NotFound because the child is statically "unfindable"
            // because blocks with this CID cannot exist.
            return Err(
                syscall_error!(NotFound; "block links to CID with forbidden codec {codec}").into(),
            );
        }

        if cid.hash().code() == fvm_shared::IDENTITY_HASH {
            // TODO: Test max recursion depth. Each level should take 6-7 bytes
            // leaving at most 11 (likely less) recursive calls (max of a 64
            // byte digest). We need to make sure this isn't going to be a
            // problem, or rewrite this to be non-recursive.
            return scan_for_links_inner(self, cid.codec(), cid.hash().digest());
        }

        if cid.hash().code() != BLAKE2B_256 || cid.hash().size() != 32 {
            return Err(syscall_error!(
                NotFound; "block links to CID with forbidden multihash type (code: {}, len: {})",
                cid.hash().code(), cid.hash().size()
            )
            .into());
        }

        // TODO: Charge a memory retention fee here? Or bundle that into the CID charge
        // above?
        self.links.push(*cid);
        Ok(())
    }
}

fn scan_for_links_inner(visitor: &mut LinkVisitor, codec: u64, data: &[u8]) -> Result<()> {
    match codec {
        DAG_CBOR => cbor::scan_for_reachable_links(visitor, data),
        IPLD_RAW | CBOR => Ok(()),
        codec => Err(syscall_error!(IllegalCodec; "codec {} not allowed", codec).into()),
    }
}

/// Scan for reachable links in the given IPLD block.
pub fn scan_for_reachable_links(
    codec: u64,
    data: &[u8],
    price_list: &PriceList,
    gas_tracker: &GasTracker,
) -> Result<Vec<Cid>> {
    let start = GasTimer::start();
    let mut visitor = LinkVisitor::new(price_list, gas_tracker.gas_available());
    let ret = scan_for_links_inner(&mut visitor, codec, data);
    let t = gas_tracker.charge_gas("OnScanIpldLinks", visitor.gas_used())?;
    let ret = ret.map(|_| visitor.finish());
    t.stop_with(start);
    ret
}

#[cfg(test)]
mod test {
    use crate::gas::{price_list_by_network_version, Gas, GasTracker};

    use crate::kernel::{ExecutionError, Result};
    use cid::Cid;
    use fvm_ipld_encoding::{CBOR, DAG_CBOR, IPLD_RAW};
    use fvm_shared::commcid::FIL_COMMITMENT_UNSEALED;
    use fvm_shared::version::NetworkVersion;
    use multihash::{Multihash, MultihashDigest};
    use num_traits::Zero;
    use serde::{Deserialize, Serialize};

    fn scan_for_links(
        codec: u64,
        data: &[u8],
        cbor_field_count: u32,
        cbor_link_count: u32,
    ) -> Result<Vec<Cid>> {
        let mut price_list = price_list_by_network_version(NetworkVersion::V21).clone();
        // We need to pick these gas numbers such that we are unlikely to "land" on the correct gas
        // value if we get an unexpected combinations of fields/CIDs.
        price_list.ipld_cbor_scan_per_field = Gas::new(1);
        price_list.ipld_cbor_scan_per_cid = Gas::new(1 << 16);

        let expected_gas = price_list.ipld_cbor_scan_per_field * cbor_field_count
            + price_list.ipld_cbor_scan_per_cid * cbor_link_count;
        let tracker = GasTracker::new(expected_gas, Gas::zero(), false);
        let res = super::scan_for_reachable_links(codec, data, &price_list, &tracker);
        assert!(
            tracker.gas_available().is_zero(),
            "expected to run out of gas"
        );
        res
    }

    #[derive(Serialize, Deserialize)]
    struct Test(u64, Cid, u64);

    #[test]
    fn skip_raw() {
        assert!(scan_for_links(IPLD_RAW, &[1, 2, 3], 0, 0)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn basic_cbor() {
        let test_cid = Cid::new_v1(IPLD_RAW, multihash::Code::Blake2b256.digest(b"foobar"));

        let data = fvm_ipld_encoding::to_vec(&Test(0, test_cid, 1)).unwrap();

        // We find a link when looking at it as dag-cbor.
        assert_eq!(
            vec![test_cid],
            scan_for_links(DAG_CBOR, &data, 4, 1).unwrap()
        );

        // We ignore the link if it's regular CBOR.
        assert!(scan_for_links(CBOR, &data, 0, 0).unwrap().is_empty());

        // Raw also ignores the link.
        assert!(scan_for_links(IPLD_RAW, &data, 0, 0).unwrap().is_empty());

        // Can run out of gas.
        assert!(matches!(
            scan_for_links(DAG_CBOR, &data, 4, 0).unwrap_err(),
            ExecutionError::OutOfGas
        ));
    }

    #[test]
    fn recursive_cbor() {
        let test_cid = Cid::new_v1(IPLD_RAW, multihash::Code::Blake2b256.digest(b"foobar"));
        let inlined_data = fvm_ipld_encoding::to_vec(&Test(0, test_cid, 1)).unwrap();
        let inline_cid = Cid::new_v1(DAG_CBOR, Multihash::wrap(0, &inlined_data).unwrap());
        let data = fvm_ipld_encoding::to_vec(&Test(0, inline_cid, 1)).unwrap();

        assert_eq!(
            vec![test_cid],
            scan_for_links(DAG_CBOR, &data, 8, 2).unwrap()
        );
    }

    #[test]
    fn ignores_pieces() {
        let test_cid = Cid::new_v1(
            FIL_COMMITMENT_UNSEALED,
            multihash::Code::Blake2b256.digest(b"foobar"),
        );
        let data = fvm_ipld_encoding::to_vec(&Test(0, test_cid, 1)).unwrap();
        assert!(scan_for_links(DAG_CBOR, &data, 4, 1).unwrap().is_empty());
    }
}
