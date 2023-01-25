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

pub mod fevm_features;

use cucumber::World;
use fevm_features::bank_account::BankAccountWorld;
use fevm_features::recursive_call::RecursiveCallWorld;
use fevm_features::simple_coin::SimpleCoinWorld;

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
