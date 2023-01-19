//! Cucumber tests for FEVM integration test scenarios.
//!
//! See https://cucumber-rs.github.io/cucumber/current/quickstart.html
use cucumber::World;

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

/// Create constructors for a smart contract, injecting a mock provider for the client,
/// because we are not going to send them to an actual blockchain.
macro_rules! contract_constructors {
    ($contract:ident) => {
        pub fn new_with_eth_address(
            owner: fil_actor_evm::interpreter::address::EthAddress,
        ) -> $contract<ethers::providers::Provider<ethers::providers::MockProvider>> {
            // The owner of the contract is expected to be the 160 bit hash used on Ethereum.
            let address = ethers::core::types::Address::from_slice(owner.as_ref());
            // A dummy client that we don't intend to use to call the contract or send transactions.
            let (client, _mock) = ethers::providers::Provider::mocked();
            $contract::new(address, std::sync::Arc::new(client))
        }

        pub fn new_with_actor_id(
            owner: fvm_shared::ActorID,
        ) -> $contract<ethers::providers::Provider<ethers::providers::MockProvider>> {
            new_with_eth_address(fil_actor_evm::interpreter::address::EthAddress::from_id(
                owner,
            ))
        }
    };
}

mod simple_coin_world {
    use evm_contracts::simple_coin::SimpleCoin;

    contract_constructors!(SimpleCoin);

    fn dummy() {
        let _contract = new_with_actor_id(100);
        //let call = contract.send_coin(receiver, amount)
    }
}
