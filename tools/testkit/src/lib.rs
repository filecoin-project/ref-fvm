// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

#![allow(dead_code)]

use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::tester::{Account as TAccount, Tester};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;

pub mod bundle;
pub mod fevm;

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
pub struct Account {
    pub account: TAccount,
    pub seqno: u64,
}

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

pub fn create_account(tester: &mut BasicTester) -> Account {
    let accounts: [TAccount; 1] = tester.create_accounts().unwrap();
    Account {
        account: accounts[0],
        seqno: 0,
    }
}

pub fn create_accounts<const N: usize>(tester: &mut BasicTester) -> [Account; N] {
    let accounts: [TAccount; N] = tester.create_accounts().unwrap();
    accounts.map(|a| Account {
        account: a,
        seqno: 0,
    })
}

pub fn prepare_execution(tester: &mut BasicTester, options: &ExecutionOptions) {
    tester
        .instantiate_machine_with_config(
            DummyExterns,
            |cfg| cfg.actor_debugging = options.debug,
            |mc| mc.tracing = options.trace,
        )
        .unwrap();
}

pub fn exit_with_error(msg: String) -> ! {
    println!("{}", msg);
    std::process::exit(1);
}
