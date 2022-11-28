use fvm_integration_tests::bundle;
use fvm_ipld_blockstore::Blockstore;
use cid::Cid;
use anyhow::anyhow;
use std::fs;

pub fn import_bundle(blockstore: &impl Blockstore, path: &str) -> anyhow::Result<Cid> {
    let bundle_data = match fs::read(path) {
        Ok(data) => data,
        Err(what) => {return Err(anyhow!("error reading bundle: {}", what));}
    };
    bundle::import_bundle(&blockstore, &bundle_data)
}
