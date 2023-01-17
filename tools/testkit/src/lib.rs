#![allow(dead_code)]

use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::tester::Tester;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;

pub mod bundle;
//pub mod fevm;

/// Execution options
pub struct ExecutionOptions {
    /// Enables debug logging
    pub debug: bool,
    /// Enables gas tracing
    pub trace: bool,
    /// Enabls events
    pub events: bool,
}

pub type BasicTester = Tester<MemoryBlockstore, DummyExterns>;

pub fn new_tester(bundle_path: &String) -> BasicTester {
    let blockstore = MemoryBlockstore::default();
    let bundle_cid = match bundle::import_bundle(&blockstore, bundle_path.as_str()) {
        Ok(cid) => cid,
        Err(what) => {
            exit_with_error(format!("error loading bundle: {}", what));
        }
    };
    Tester::new(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        bundle_cid,
        blockstore,
    )
    .unwrap_or_else(|what| {
        exit_with_error(format!("error creating execution framework: {}", what));
    })
}

pub fn exit_with_error(msg: String) -> ! {
    println!("{}", msg);
    std::process::exit(1);
}
