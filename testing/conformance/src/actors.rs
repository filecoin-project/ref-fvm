// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::sync::Mutex;

use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
use fvm_ipld_car::load_car;
use lazy_static::lazy_static;

static V9_BUNDLE: &[u8] = include_bytes!("../actors/v9.tar.zst");

lazy_static! {
    static ref ACTORS: Mutex<MemoryBlockstore> =
        Mutex::new(load_bundles(&[V9_BUNDLE]).expect("failed to load bundles"));
}

fn load_bundles(bundles: &[&[u8]]) -> anyhow::Result<MemoryBlockstore> {
    let bs = MemoryBlockstore::new();
    for bundle in bundles {
        let mut reader = tar::Archive::new(zstd::Decoder::with_buffer(*bundle)?);
        for entry in reader.entries()? {
            load_car(&bs, entry?)?;
        }
    }
    Ok(bs)
}

/// Load the bundled actors into the specified blockstore.
pub fn load_actors(bs: &impl Blockstore) -> anyhow::Result<()> {
    ACTORS.lock().unwrap().copy_to(bs)
}
