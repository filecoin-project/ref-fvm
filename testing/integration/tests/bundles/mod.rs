use std::collections::BTreeMap;

use anyhow::Context;
use fvm::externs::Externs;
use fvm_integration_tests::bundle;
use fvm_integration_tests::tester::Tester;
use fvm_ipld_blockstore::Blockstore;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use lazy_static::lazy_static;

lazy_static! {
    static ref BUNDLES: BTreeMap<NetworkVersion, &'static [u8]> = [
        (NetworkVersion::V15, actors_v9::BUNDLE_CAR),
        (NetworkVersion::V16, actors_v9::BUNDLE_CAR), // todo bad hack
    ].into_iter().collect();
}

pub fn new_tester<B: Blockstore, E: Externs>(
    nv: NetworkVersion,
    stv: StateTreeVersion,
    blockstore: B,
) -> anyhow::Result<Tester<B, E>> {
    let bundle = BUNDLES
        .get(&nv)
        .with_context(|| format!("unsupported network version {nv}"))?;
    let root = bundle::import_bundle(&blockstore, bundle)?;
    Tester::new(nv, stv, root, blockstore)
}
