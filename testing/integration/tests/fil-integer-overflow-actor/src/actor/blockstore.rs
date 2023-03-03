// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::{anyhow, Result};
use cid::Cid;
use fvm_ipld_blockstore::Block;
use fvm_sdk as sdk;

/// A blockstore that delegates to IPLD syscalls.
pub struct Blockstore;

impl fvm_ipld_blockstore::Blockstore for Blockstore {
    fn get(&self, cid: &Cid) -> Result<Option<Vec<u8>>> {
        // If this fails, the _CID_ is invalid. I.e., we have a bug.
        sdk::ipld::get(cid)
            .map(Some)
            .map_err(|e| anyhow!("get failed with {:?} on CID '{}'", e, cid))
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<()> {
        let k2 = self.put(k.hash().code(), &(k.codec(), block))?;
        if k != &k2 {
            return Err(anyhow!("put block with cid {} but has cid {}", k, k2));
        }
        Ok(())
    }

    fn put(&self, mh_code: u64, block: &dyn Block) -> Result<Cid> {
        // TODO: Don't hard-code the size. Unfortunately, there's no good way to get it from the
        //  codec at the moment.
        const SIZE: u32 = 32;
        let k = sdk::ipld::put(mh_code, SIZE, block.codec(), block.data())
            .map_err(|e| anyhow!("put failed with {:?}", e))?;
        Ok(k)
    }
}
