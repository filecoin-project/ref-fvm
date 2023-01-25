// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! Cucumber tests for FEVM integration test scenarios.
//!
//! See https://cucumber-rs.github.io/cucumber/current/quickstart.html
//!
//! Example:
//! ```text
//! cargo test --release --test fevm
//! ```

use std::collections::BTreeMap;

use cucumber::World;
use lazy_static::lazy_static;

pub mod bank_account;
pub mod common;
pub mod recursive_call;
pub mod simple_coin;

use bank_account::BankAccountWorld;
use recursive_call::RecursiveCallWorld;
use simple_coin::SimpleCoinWorld;

/// Used once to load contracts from files.
macro_rules! contract_sources {
    ($($sol:literal / $contract:literal),+) => {
        [ $((($sol, $contract), include_str!(concat!("../evm/artifacts/", $sol, ".sol/", $contract, ".hex")))),+ ]
    };
}

lazy_static! {
    /// Pre-loaded contract code bytecode in hexadecimal format.
    static ref CONTRACTS: BTreeMap<(&'static str, &'static str), Vec<u8>> = contract_sources! {
                "SimpleCoin" / "SimpleCoin",
                "RecursiveCall" / "RecursiveCall",
                "BankAccount" / "Bank",
                "BankAccount" / "Account"
    }
    .into_iter()
    .map(|((sol, contract), code)| {
        let bz = hex::decode(&code.trim_end()).expect(&format!("error parsing {sol}/{contract}")).into();
        ((sol, contract), bz)
    })
    .collect();
}

// Using `tokio` to execute asynchronously rather than the `futures::executor::block_on`
// as in the Cucumber book because `bundles::import_bundle` also uses `block_on`,
// which doesn't work, because it would deadlock on the single threaded `LocalPool`.
#[tokio::main]
async fn main() {
    // NOTE: Enable `fail_on_skipped` or `repeat_skipped` if there are too many scenarios:
    //  https://cucumber-rs.github.io/cucumber/current/writing/tags.html#failing-on-skipped-steps
    SimpleCoinWorld::run("tests/evm/features/SimpleCoin.feature").await;
    RecursiveCallWorld::run("tests/evm/features/RecursiveCall.feature").await;
    BankAccountWorld::run("tests/evm/features/BankAccount.feature").await;
}
