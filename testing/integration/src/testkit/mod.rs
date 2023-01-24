// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::Result;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;

use crate::dummy::DummyExterns;
use crate::tester::{Account, Tester};

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

/// This is an account wrapper to allow tracking of the current nonce associated with
/// an account.
// TOOO the nonce can be pused into the bae account type, but that's a larger refactoring
// we avoid  iat first pass.
pub struct BasicAccount {
    pub account: Account,
    pub seqno: u64,
}

impl BasicTester {
    pub fn new_tester(bundle_path: String) -> Result<BasicTester> {
        let blockstore = MemoryBlockstore::default();
        let bundle_cid = match bundle::import_bundle(&blockstore, bundle_path.as_str()) {
            Ok(cid) => cid,
            Err(what) => return Err(what),
        };

        Tester::new(
            NetworkVersion::V18,
            StateTreeVersion::V5,
            bundle_cid,
            blockstore,
        )
    }

    // must be called after accounts have been created and the machine is ready to run
    fn prepare_execution(&mut self, options: &ExecutionOptions) -> Result<()> {
        self.instantiate_machine_with_config(
            DummyExterns,
            |cfg| cfg.actor_debugging = options.debug,
            |mc| mc.tracing = options.trace,
        )
    }

    // TODO this method should move to the basie type. once the accounts have been integrated
    pub fn create_basic_account(&mut self, options: &ExecutionOptions) -> Result<BasicAccount> {
        let accounts: [Account; 1] = self.create_accounts().unwrap();
        let account = BasicAccount {
            account: accounts[0],
            seqno: 0,
        };
        self.prepare_execution(options)?;
        Ok(account)
    }

    // TODO base type has the method, we need this to create the account wrapper; should go
    //      away once the latter hsa been integrated.
    pub fn create_basic_accounts<const N: usize>(
        &mut self,
        options: &ExecutionOptions,
    ) -> Result<[BasicAccount; N]> {
        let accounts: [Account; N] = self.create_accounts().unwrap();
        let accounts = accounts.map(|a| BasicAccount {
            account: a,
            seqno: 0,
        });
        self.prepare_execution(options)?;
        Ok(accounts)
    }
}
