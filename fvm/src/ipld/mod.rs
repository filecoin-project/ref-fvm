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
    fn new(price_list: &'a PriceList, gas_available: Gas) -> Self {
        Self {
            price_list,
            gas_available,
            gas_remaining: gas_available,
            links: Vec::new(),
        }
    }

    fn finish(self) -> Vec<Cid> {
        self.links
    }

    fn gas_used(&self) -> Gas {
        self.gas_available - self.gas_remaining
    }

    /// Charge for gas used, returning an error if we run out.
    fn charge_gas(&mut self, gas: Gas) -> Result<()> {
        if self.gas_remaining < gas {
            self.gas_remaining = Gas::zero();
            return Err(ExecutionError::OutOfGas);
        }
        self.gas_remaining -= gas;
        Ok(())
    }

    /// Visit a CID, possibly ignoring it or even recursively scanning it.
    /// - This function will recursively scan "inline" blocks (identity-hashed CIDs) for recursive
    ///   links, but won't return inline CIDs directly.
    /// - This function will ignore valid Filecoin sector CIDs.
    /// - This function will reject blocks that link to blocks with unsupported codecs.
    fn visit_cid(&mut self, cid: &Cid) -> Result<()> {
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
    let t = gas_tracker.charge_gas("ScanIpldLinks", visitor.gas_used())?;
    t.stop_with(start);
    ret.map(|_| visitor.finish())
}
