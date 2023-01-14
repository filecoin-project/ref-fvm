// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use cid::Cid;
use fvm_ipld_blockstore::Blockstore;

// A blockstore that accepts but discards all insertions, and returns errors on reads.
// Useful for when the FVM needs to stage ephemeral data structures without persisting them,
// like the events AMT.
pub struct DiscardBlockstore;

impl Blockstore for DiscardBlockstore {
    fn get(&self, _: &Cid) -> anyhow::Result<Option<Vec<u8>>> {
        Err(anyhow::anyhow!(
            "Blockstore#get not supported with DiscardBlockstore"
        ))
    }

    fn put_keyed(&self, _: &Cid, _: &[u8]) -> anyhow::Result<()> {
        Ok(())
    }
}
