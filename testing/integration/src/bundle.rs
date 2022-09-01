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
