//! Cucumber tests for FEVM integration test scenarios.
//!
//! See https://cucumber-rs.github.io/cucumber/current/quickstart.html

use std::collections::BTreeMap;
use std::fmt::Display;
use std::str::FromStr;

use cucumber::{Parameter, World};
use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::fevm::{Account, BasicTester, CreateReturn};
use fvm_integration_tests::tester::Account as TestAccount;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_shared::address::Address;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use lazy_static::lazy_static;
use simple_coin_world::SimpleCoinWorld;

mod bundles;

/// Used once to load contracts from files.
macro_rules! contract_sources {
    ($($name:literal),+) => {
        $(($name, include_str!(concat!("evm/artifacts/", $name, ".sol/", $name, ".hex")))),+
    };
}

lazy_static! {
    /// Pre-loaded contract code bytecode in hexadecimal format.
    static ref CONTRACTS: BTreeMap<&'static str, Vec<u8>> = [contract_sources! {
                "SimpleCoin"
    }]
    .into_iter()
    .map(|(name, code)| {
        let bz = hex::decode(&code.trim_end()).expect(&format!("error parsing {name}")).into();
        (name, bz)
    })
    .collect();
}

// Using `tokio` to execute asynchronously rather than the `futures::executor::block_on`
// as in the Cucumber book because `bundles::import_bundle` also uses `block_on`,
// which doesn't work, because it would deadlock on the single threaded `LocalPool`.
#[tokio::main]
async fn main() {
    SimpleCoinWorld::run("tests/evm/features/SimpleCoin.feature").await;
}

/// Get a contract from the pre-loaded sources.
pub fn get_contract_code(name: &str) -> &[u8] {
    CONTRACTS
        .get(name)
        .ok_or_else(|| format!("contract {name} hasn't been loaded"))
        .unwrap()
}

/// Account number that's +1 from array indexes, e.g. `account 1` is in `accounts[0]`.
///
/// This can be used in Gherkin like `When account 1 sends 10 tokens to account 2`.
///
/// After parsing, the value inside is the array index without having to -1.
#[derive(Parameter)]
#[param(name = "acct", regex = r"account (\d+)")]
pub struct AccountNumber(pub usize);

impl FromStr for AccountNumber {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match usize::from_str(s) {
            Ok(0) => Err("AccountNumber has to be at minimum 1".to_owned()),
            Ok(n) => Ok(AccountNumber(n - 1)),
            Err(_) => Err(format!("not an integer: {s}")),
        }
    }
}

impl Display for AccountNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "account {}", self.0 + 1)
    }
}

/// Common machinery for all worlds to created and call contracts.
pub struct ContractTester {
    tester: BasicTester,
    accounts: Vec<Account>,
}

impl Default for ContractTester {
    fn default() -> Self {
        Self::new(NetworkVersion::V18, StateTreeVersion::V5)
    }
}

impl std::fmt::Debug for ContractTester {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContractTester")
            .field("accounts", &self.accounts)
            .finish()
    }
}

impl ContractTester {
    pub fn new(nv: NetworkVersion, stv: StateTreeVersion) -> Self {
        let blockstore = MemoryBlockstore::default();
        let tester = match bundles::new_tester(nv, stv, blockstore) {
            Ok(t) => t,
            Err(e) => panic!("error creating tester with NV={nv} and STV={stv:?}: {e}"),
        };
        Self {
            tester,
            accounts: Vec::new(),
        }
    }

    /// Prime the machine for contract execution.
    ///
    /// Note that it's not possible to create more accounts after this.
    fn instantiate_machine(&mut self) {
        self.tester
            .instantiate_machine(DummyExterns)
            .expect("error instantiating machine");
    }

    /// Make sure the manchine has been primed.
    fn ensure_machine_instantiated(&mut self) {
        if self.tester.executor.is_none() {
            self.instantiate_machine()
        }
    }

    /// Create `n` random accounts.
    pub fn create_accounts(&mut self, n: usize) {
        assert!(
            self.tester.state_tree.is_some(),
            "The machine has already been initialized, can't create more accounts."
        );

        for _ in 0..n {
            let accounts: [TestAccount; 1] = self
                .tester
                .create_accounts()
                .expect("error creating account");

            let account = Account {
                account: accounts[0],
                seqno: 0,
            };

            self.accounts.push(account);
        }
    }

    /// Get a mutable reference to an account
    pub fn account_mut(&mut self, acct: &AccountNumber) -> &mut Account {
        self.accounts
            .get_mut(acct.0)
            .ok_or_else(|| format!("{acct} has not been created"))
            .unwrap()
    }

    /// Deploy a contract owned by an account.
    pub fn create_contract(&mut self, owner: AccountNumber, contract_name: String) {
        self.ensure_machine_instantiated();

        // Need to clone because I have to pass 2 mutable references to `fevm::create_contract`.
        let mut account = self.account_mut(&owner).clone();
        let contract = get_contract_code(&contract_name);

        let create_res =
            fvm_integration_tests::fevm::create_contract(&mut self.tester, &mut account, contract);

        *self.account_mut(&owner) = account;

        let create_return: CreateReturn = create_res
            .msg_receipt
            .return_data
            .deserialize()
            .expect("error deserializing CreateReturn");

        let _actor_addr = Address::new_id(create_return.actor_id);
    }
}

/// Create common given-when-then matchers for a `World` that is
/// expected to have a `tester: ContractTester` field.
macro_rules! contract_matchers {
    ($world:ident) => {
        #[given(expr = "{int} random account(s)")]
        fn create_accounts(world: &mut $world, n: usize) {
            world.tester.create_accounts(n);
        }

        #[when(expr = "{acct} creates a {word} contract")]
        fn create_contract(world: &mut $world, owner: $crate::AccountNumber, contract: String) {
            world.tester.create_contract(owner, contract)
        }
    };
}

/// Create constructors for a smart contract, injecting a mock provider for the client,
/// because we are not going to send them to an actual blockchain.
// macro_rules! contract_constructors {
//     ($contract:ident) => {
//         #[allow(dead_code)]
//         pub fn new_with_eth_address(
//             owner: fil_actor_evm::interpreter::address::EthAddress,
//         ) -> $contract<ethers::providers::Provider<ethers::providers::MockProvider>> {
//             // The owner of the contract is expected to be the 160 bit hash used on Ethereum.
//             let address = ethers::core::types::Address::from_slice(owner.as_ref());
//             // A dummy client that we don't intend to use to call the contract or send transactions.
//             let (client, _mock) = ethers::providers::Provider::mocked();
//             $contract::new(address, std::sync::Arc::new(client))
//         }

//         #[allow(dead_code)]
//         pub fn new_with_actor_id(
//             owner: fvm_shared::ActorID,
//         ) -> $contract<ethers::providers::Provider<ethers::providers::MockProvider>> {
//             let owner = fil_actor_evm::interpreter::address::EthAddress::from_id(owner);
//             new_with_eth_address(owner)
//         }
//     };
// }

mod simple_coin_world {
    use cucumber::{given, when, World};

    //use evm_contracts::simple_coin::SimpleCoin;
    use crate::ContractTester;

    // contract_constructors!(SimpleCoin);

    // `World` is your shared, likely mutable state.
    // Cucumber constructs it via `Default::default()` for each scenario.
    #[derive(World, Default, Debug)]
    pub struct SimpleCoinWorld {
        pub tester: ContractTester,
    }

    contract_matchers!(SimpleCoinWorld);
}
