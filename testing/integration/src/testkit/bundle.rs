// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use std::fs;

use anyhow::anyhow;
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;

use crate::bundle;

pub fn import_bundle(blockstore: &impl Blockstore, path: &str) -> anyhow::Result<Cid> {
    let bundle_data = match fs::read(path) {
        Ok(data) => data,
        Err(what) => {
            return Err(anyhow!("error reading bundle: {}", what));
        }
    };
    bundle::import_bundle(&blockstore, &bundle_data)
}
