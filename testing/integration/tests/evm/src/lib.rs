pub mod simplecoin;

/// Create a `new` method in a module with a smart contract.
///
/// # Example
///
/// ```ignore
/// abigen!(SimpleCoin, "./artifacts/SimpleCoin.sol/SimpleCoin.abi");
///
/// new_with_mock_provider!(SimpleCoin);
/// ```
#[macro_export]
macro_rules! new_with_mock_provider {
    ($contract:ident) => {
        pub fn new(
            owner: fil_actor_evm::interpreter::address::EthAddress,
        ) -> $contract<ethers::providers::Provider<ethers::providers::MockProvider>> {
            // The owner of the contract is expected to be the 160 bit hash used on Ethereum.
            let address = ethers::core::types::Address::from_slice(owner.as_ref());
            // A dummy client that we don't intend to use to call the contract or send transactions.
            let (client, _mock) = ethers::providers::Provider::mocked();
            $contract::new(address, std::sync::Arc::new(client))
        }
    };
}
