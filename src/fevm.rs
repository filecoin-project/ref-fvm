use anyhow;
use fvm_integration_tests::tester::Tester;
use fvm_ipld_blockstore::Blockstore;
use fvm::externs::Externs;

pub fn run<B: Blockstore, E: Externs>(_tester: &mut Tester<B, E>, _contract: &[u8], _entrypoint: &[u8], _params: &[u8]) -> anyhow::Result<()> {
    Ok(())
}
