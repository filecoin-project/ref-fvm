use cid::Cid;
use anyhow::anyhow;
use fvm_integration_tests::tester::Tester;
use fvm_ipld_blockstore::Blockstore;
use fvm::externs::Externs;

pub fn run<B: Blockstore, E: Externs>(tester: &mut Tester<B, E>, contract: &[u8], entrypoint: &[u8], params: &[u8]) -> anyhow::Result<()> {
    Ok(())
}
