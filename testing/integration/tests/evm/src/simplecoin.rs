use ethers::prelude::abigen;

use crate::new_with_mock_provider;

abigen!(SimpleCoin, "./artifacts/SimpleCoin.sol/SimpleCoin.abi");
new_with_mock_provider!(SimpleCoin);
