// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::anyhow;
use cid::Cid;
use futures::executor::block_on;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_car::load_car_unchecked;

// Import built-in actors
pub fn import_bundle(blockstore: &impl Blockstore, bundle: &[u8]) -> anyhow::Result<Cid> {
    match &*block_on(async { load_car_unchecked(blockstore, bundle).await })? {
        [root] => Ok(*root),
        _ => Err(anyhow!("multiple root CIDs in bundle")),
    }
}

pub fn import_bundle_from_path(blockstore: &impl Blockstore, path: &str) -> anyhow::Result<Cid> {
    let bundle_data = match std::fs::read(path) {
        Ok(data) => data,
        Err(what) => {
            return Err(anyhow!("error reading bundle: {}", what));
        }
    };

    import_bundle(blockstore, &bundle_data)
}
