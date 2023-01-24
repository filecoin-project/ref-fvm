// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::Result;
use fvm::executor::ApplyRet;
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

pub struct BasicTester {
    inner: Tester<MemoryBlockstore, DummyExterns>,
    pub options: ExecutionOptions,
    ready: bool,
}

/// This is an account wrapper to allow tracking of the current nonce associated with
/// an account.
// TOOO the nonce can be pused into the bae account type, but that's a larger refactoring
// we avoid  iat first pass.
pub struct BasicAccount {
    pub account: Account,
    pub seqno: u64,
}

impl BasicTester {
    pub fn new(bundle_path: String, options: ExecutionOptions) -> Result<BasicTester> {
        let blockstore = MemoryBlockstore::default();
        let bundle_cid = match bundle::import_bundle(&blockstore, bundle_path.as_str()) {
            Ok(cid) => cid,
            Err(what) => return Err(what),
        };

        let inner = Tester::new(
            NetworkVersion::V18,
            StateTreeVersion::V5,
            bundle_cid,
            blockstore,
        )?;

        Ok(BasicTester {
            inner,
            options,
            ready: false,
        })
    }

    pub fn with_inner<F>(&mut self, f: F) -> Result<ApplyRet>
    where
        F: FnOnce(&mut Tester<MemoryBlockstore, DummyExterns>) -> Result<ApplyRet>,
    {
        self.prepare_execution()?;
        f(&mut self.inner)
    }

    // must be called after accounts have been created and the machine is ready to run
    fn prepare_execution(&mut self) -> Result<()> {
        if !self.ready {
            self.inner.instantiate_machine_with_config(
                DummyExterns,
                |cfg| cfg.actor_debugging = self.options.debug,
                |mc| mc.tracing = self.options.trace,
            )?;
            self.ready = true
        }
        Ok(())
    }

    // TODO this method should move to the basie type. once the accounts have been integrated
    pub fn create_account(&mut self) -> BasicAccount {
        let accounts: [Account; 1] = self.inner.create_accounts().unwrap();
        BasicAccount {
            account: accounts[0],
            seqno: 0,
        }
    }

    // TODO base type has the method, we need this to create the account wrapper; should go
    //      away once the latter hsa been integrated.
    pub fn create_basic_accounts<const N: usize>(&mut self) -> [BasicAccount; N] {
        let accounts: [Account; N] = self.inner.create_accounts().unwrap();
        accounts.map(|a| BasicAccount {
            account: a,
            seqno: 0,
        })
    }
}
