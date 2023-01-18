//! Cucumber tests for FEVM integration test scenarios.
//!
//! See https://cucumber-rs.github.io/cucumber/current/quickstart.html
use cucumber::World;
// Check that we can import.
use evm_contracts::simplecoin::SimpleCoin;

/// Cucumber constructs it via `Default::default()` for each scenario.
#[derive(Debug, Default, World)]
pub struct FevmWorld {}

// This runs before everything else, so you can setup things here.
fn main() {
    // You may choose any executor you like (`tokio`, `async-std`, etc.).
    // You may even have an `async` main.
    // We can run the features of each contract separately if they need
    // different `World` implementations.
    futures::executor::block_on(FevmWorld::run("tests/evm/features"));
}
