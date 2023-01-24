// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! Cucumber tests for FEVM integration test scenarios.
//!
//! See https://cucumber-rs.github.io/cucumber/current/quickstart.html

use std::collections::BTreeMap;
use std::fmt::Display;
use std::str::FromStr;

use bank_account::BankAccountWorld;
use cucumber::gherkin::Step;
use cucumber::{Parameter, World};
use ethers::abi::Detokenize;
use ethers::prelude::builders::ContractCall;
use ethers::prelude::{decode_function_data, AbiError};
use ethers::types::{Bytes, H160, H256};
use fvm::executor::ApplyFailure;
use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::fevm::{Account, BasicTester, CreateReturn, EthAddress};
use fvm_integration_tests::tester::{Account as TestAccount, INITIAL_ACCOUNT_BALANCE};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::BytesDe;
use fvm_shared::address::Address;
use fvm_shared::error::ExitCode;
use fvm_shared::event::StampedEvent;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use fvm_shared::ActorID;
use lazy_static::lazy_static;
use libsecp256k1::SecretKey;
use recursive_call_world::RecursiveCallWorld;
use simple_coin_world::SimpleCoinWorld;

mod bundles;

/// Used once to load contracts from files.
macro_rules! contract_sources {
    ($($sol:literal / $contract:literal),+) => {
        [ $(($contract, include_str!(concat!("evm/artifacts/", $sol, ".sol/", $contract, ".hex")))),+ ]
    };
}

lazy_static! {
    /// Pre-loaded contract code bytecode in hexadecimal format.
    ///
    /// Assumes all the contract names are unique across all files!
    static ref CONTRACTS: BTreeMap<&'static str, Vec<u8>> = contract_sources! {
                "SimpleCoin" / "SimpleCoin",
                "RecursiveCall" / "RecursiveCall",
                "BankAccount" / "Bank",
                "BankAccount" / "Account"
    }
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
    // NOTE: Enable `fail_on_skipped` or `repeat_skipped` if there are too many scenarios:
    //  https://cucumber-rs.github.io/cucumber/current/writing/tags.html#failing-on-skipped-steps
    SimpleCoinWorld::run("tests/evm/features/SimpleCoin.feature").await;
    RecursiveCallWorld::run("tests/evm/features/RecursiveCall.feature").await;
    BankAccountWorld::run("tests/evm/features/BankAccount.feature").await;
}

/// Get a contract from the pre-loaded sources.
pub fn get_contract_code(name: &str) -> &[u8] {
    CONTRACTS
        .get(name)
        .ok_or_else(|| format!("contract {name} hasn't been loaded"))
        .unwrap()
}

/// Gas that should be enough to call anything.
pub const DEFAULT_GAS: i64 = 10_000_000_000i64;

/// Account number that's +1 from array indexes, e.g. `account 1` is in `accounts[0]`.
///
/// This can be used in Gherkin like `When account 1 sends 10 tokens to account 2`.
///
/// After parsing, the value inside is the array index without having to -1.
#[derive(Parameter, Debug, Clone, Copy)]
#[param(name = "acct", regex = r"account (\d+)")]
pub struct AccountNumber(pub usize);

impl FromStr for AccountNumber {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match usize::from_str(s.strip_prefix("account ").unwrap_or(s)) {
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

/// Contract number that's +1 from array indexes, e.g. `contract 1` is in `contracts[0]`.
///
/// This can be used in Gherkin like `When account 1 calls contract 2 ...`.
///
/// After parsing, the value inside is the array index without having to -1.
#[derive(Parameter, Debug, Clone, Copy)]
#[param(name = "cntr", regex = r"contract (\d+)")]
pub struct ContractNumber(pub usize);

impl FromStr for ContractNumber {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match usize::from_str(s.strip_prefix("contract ").unwrap_or(s)) {
            Ok(0) => Err("ContractNumber has to be at minimum 1".to_owned()),
            Ok(n) => Ok(ContractNumber(n - 1)),
            Err(_) => Err(format!("not an integer: {s}")),
        }
    }
}

impl Display for ContractNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "contract {}", self.0 + 1)
    }
}

/// Remember what contract was deployed.
#[derive(Debug, Clone)]
pub struct DeployedContract {
    /// Name would be useful if we had multiple contracts in the same solidity file
    /// and wanted to check what contract was deployed at a certain slot.
    _name: String,
    owner: TestAccount,
    /// The ActorID address.
    address: Address,
    /// The ethereum address from `CreateReturn`, produced by the EAM actor.
    eth_address: EthAddress,
}

impl DeployedContract {
    pub fn addr_to_h160(&self) -> H160 {
        id_to_h160(self.address.id().expect("contract address is an ID"))
    }
    pub fn owner_id(&self) -> ActorID {
        self.owner.0
    }
}

/// Error info returned in `ApplyRet`..
#[derive(Debug)]
pub struct ExecError {
    pub exit_code: ExitCode,
    pub failure_info: Option<ApplyFailure>,
}

/// Common machinery for all worlds to created and call contracts.
pub struct ContractTester {
    tester: BasicTester,
    /// Accounts created by the tester.
    accounts: Vec<Account>,
    /// Contracts created by the tester; `(owner, contract_address)`.
    contracts: Vec<DeployedContract>,
    /// Events emitted by the last contract invocation.
    last_events: Vec<StampedEvent>,
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
            contracts: Vec::new(),
            last_events: Vec::new(),
        }
    }

    /// Prime the machine for contract execution.
    ///
    /// Note that it's not possible to create more accounts after this.
    fn instantiate_machine(&mut self) {
        self.tester
            .instantiate_machine_with_config(
                DummyExterns,
                |nc| {
                    // Disable this because it's mixed with test output and repetitive.
                    nc.actor_debugging = false
                },
                |_mc| {},
            )
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

    /// Create accounts with the given list of private keys.
    pub fn create_accounts_with_keys(&mut self, step: &Step) {
        if let Some(table) = step.table.as_ref() {
            // NOTE: skip header
            for row in table.rows.iter().skip(1) {
                let priv_key = &row[0];
                let priv_key = hex::decode(priv_key).expect("invalid private key");
                let priv_key =
                    SecretKey::parse_slice(&priv_key).expect("invalid Secp256k1 private key");
                let account = self
                    .tester
                    .make_secp256k1_account(priv_key, INITIAL_ACCOUNT_BALANCE.clone())
                    .expect("error creating account");

                let account = Account { account, seqno: 0 };

                self.accounts.push(account);
            }
        }
    }

    /// Get a mutable reference to an account.
    pub fn account_mut(&mut self, acct: AccountNumber) -> &mut Account {
        self.accounts
            .get_mut(acct.0)
            .ok_or_else(|| format!("{acct} has not been created"))
            .unwrap()
    }

    /// Get a reference to a created account.
    pub fn account(&self, acct: AccountNumber) -> &Account {
        self.accounts
            .get(acct.0)
            .ok_or_else(|| format!("{acct} has not been created"))
            .unwrap()
    }

    /// Get the ID of an account we created earlier.
    pub fn account_id(&self, acct: AccountNumber) -> ActorID {
        self.account(acct).account.0
    }

    /// Address type expected by the `ethers` ABI generated code.
    pub fn account_h160(&self, acct: AccountNumber) -> H160 {
        id_to_h160(self.account_id(acct))
    }

    /// Deploy a contract owned by an account.
    pub fn create_contract(
        &mut self,
        owner: AccountNumber,
        contract_name: String,
    ) -> Result<(), ExecError> {
        self.ensure_machine_instantiated();

        // Need to clone because I have to pass 2 mutable references to `fevm::create_contract`.
        let mut account = self.account_mut(owner).clone();
        let creator = account.account;
        let contract = get_contract_code(&contract_name);

        let create_res =
            fvm_integration_tests::fevm::create_contract(&mut self.tester, &mut account, contract);

        *self.account_mut(owner) = account;

        if !create_res.msg_receipt.exit_code.is_success() {
            return Err(ExecError {
                exit_code: create_res.msg_receipt.exit_code,
                failure_info: create_res.failure_info,
            });
        }

        let create_return: CreateReturn = create_res
            .msg_receipt
            .return_data
            .deserialize()
            .expect("error deserializing CreateReturn");

        let contract_addr = Address::new_id(create_return.actor_id);

        let contract = DeployedContract {
            _name: contract_name,
            owner: creator,
            address: contract_addr,
            eth_address: create_return.eth_address,
        };

        self.contracts.push(contract);

        Ok(())
    }

    /// Get a previously deployed contract.
    pub fn deployed_contract(&self, cntr: ContractNumber) -> &DeployedContract {
        self.contracts
            .get(cntr.0)
            .ok_or_else(|| format!("{cntr} has not been created"))
            .unwrap()
    }

    /// Instantiate the last created contract and return it with its address.
    pub fn last_contract<T, F>(&self, f: F) -> (T, Address)
    where
        F: Fn(ActorID) -> T,
    {
        let deployed = self
            .contracts
            .last()
            .expect("haven't deployed a contract yet");

        let contract = f(deployed.owner_id());
        (contract, deployed.address)
    }

    /// Instantiate a contract by number.
    pub fn contract<T, F>(&self, cntr: ContractNumber, f: F) -> (T, Address)
    where
        F: Fn(ActorID) -> T,
    {
        let deployed = self.deployed_contract(cntr);
        let contract = f(deployed.owner_id());
        (contract, deployed.address)
    }

    /// Take a ABI method call, with the caller and the destination address.
    /// Then wrap it up into a message (ie. transaction) and run it through
    /// the execution stack. Finally parse the results.
    pub fn call_contract<R: Detokenize>(
        &mut self,
        acct: AccountNumber,
        contract_addr: Address,
        call: TestContractCall<R>,
    ) -> Result<R, ExecError> {
        let input = call.calldata().expect("Should have calldata.");
        let mut account = self.account_mut(acct).clone();
        let invoke_res = fvm_integration_tests::fevm::invoke_contract(
            &mut self.tester,
            &mut account,
            contract_addr,
            &input,
            call.tx
                .gas()
                .expect("need to set gas")
                .as_u64()
                .try_into()
                .expect("too much gas"),
        );

        // I think the nonce doesn't need to increase for views, but
        // maybe that's just an optimisation by actually using a local node.
        // FWIW the system increases the seqno, it doesn't have a special
        // relationship with the EVM actor.
        // NB `call.function.state_mutability` would tell us.
        *self.account_mut(acct) = account;

        // Store events, they can be parsed by the world that knows what to expect.
        self.last_events.clear();
        for evt in invoke_res.events {
            self.last_events.push(evt)
        }

        if !invoke_res.msg_receipt.exit_code.is_success() {
            return Err(ExecError {
                exit_code: invoke_res.msg_receipt.exit_code,
                failure_info: invoke_res.failure_info,
            });
        }

        let BytesDe(bytes) = invoke_res
            .msg_receipt
            .return_data
            .deserialize()
            .expect("error deserializing return data");

        Ok(decode_function_data(&call.function, bytes, false)
            .expect("error deserializing return data"))
    }

    /// Parse the events from the last contract invocation.
    ///
    /// TODO: Add a filter for selecting the event by its type signature.
    ///
    /// The call returns events like these:
    ///
    /// ```text
    /// StampedEvent { emitter: 103,
    ///  event: ActorEvent { entries: [
    ///    Entry { flags: FLAG_INDEXED_VALUE, key: "topic1", value: RawBytes { 5820ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef } },
    ///    Entry { flags: FLAG_INDEXED_VALUE, key: "topic2", value: RawBytes { 54ff00000000000000000000000000000000000065 } },
    ///    Entry { flags: FLAG_INDEXED_VALUE, key: "topic3", value: RawBytes { 54ff00000000000000000000000000000000000066 } },
    ///    Entry { flags: FLAG_INDEXED_VALUE, key: "data", value: RawBytes { 582000000000000000000000000000000000000000000000000000000000000007d0 } }] } }
    /// ```
    ///
    /// The values are:
    /// * topic1 will be the cbor encoded keccak-256 hash of the event signature Transfer(address,address,uint256)
    /// * topic2 will be the first indexed argument, i.e. _from  (cbor encoded byte array; needs padding to 32 bytes to work with ethers)
    /// * topic3 will be the second indexed argument, i.e. _to (cbor encoded byte array; needs padding to 32 bytes to work with ethers)
    /// * data is a cbor encoded byte array of all the remaining arguments
    pub fn parse_events<F, T>(&self, contract_addr: Address, f: F) -> Vec<T>
    where
        F: Fn(Vec<H256>, Bytes) -> Result<T, AbiError>,
    {
        let contract_id = contract_addr.id().expect("contract address is an ID");
        let mut events = Vec::new();

        for event in self.last_events.iter() {
            if event.emitter == contract_id {
                let mut topics = Vec::<H256>::new();
                let entries_len = event.event.entries.len();

                for entry in event.event.entries.iter().take(entries_len - 1) {
                    let BytesDe(topic) = entry
                        .value
                        .deserialize()
                        .expect("error deserializing topic entry");
                    let topic = to_h256(&topic);
                    topics.push(topic)
                }

                let BytesDe(data) = event.event.entries[entries_len - 1]
                    .value
                    .deserialize()
                    .expect("error deserializing data entry");
                let data = Bytes::from(data);

                let event: T = f(topics, data).expect("error decoding event");

                events.push(event);
            }
        }
        events
    }
}

/// Create common given-when-then matchers for a `World` that is
/// expected to have a `tester: ContractTester` field.
///
/// Make sure these imports are in scope:
///
/// ```ignore
/// use cucumber::gherkin::Step;
/// use cucumber::{given, then, when, World};
/// ```
macro_rules! contract_matchers {
    ($world:ident) => {
        /// Example:
        /// ```text
        /// Given 3 random accounts`
        /// ```
        #[given(expr = "{int} random account(s)")]
        fn create_accounts(world: &mut $world, n: usize) {
            world.tester.create_accounts(n);
        }

        /// Example:
        /// ```text
        /// Given accounts with private keys
        ///   | private keys                                                     |
        ///   | 5e969c4ac2f287128d6fd71e7d111dbd19a5b2bea59da5d5d908044a514f5f8e |
        ///   | ...                                                              |
        /// ```
        #[given(expr = "accounts with private keys")]
        fn create_accounts_with_keys(world: &mut $world, step: &Step) {
            world.tester.create_accounts_with_keys(step);
        }

        /// Example:
        /// ```text
        /// When account 1 creates a SimpleCoin contract
        /// ```
        #[when(expr = "{acct} creates a {word} contract")]
        fn create_contract(world: &mut $world, owner: $crate::AccountNumber, contract: String) {
            world
                .tester
                .create_contract(owner, contract)
                .expect("countract creation should succeed")
        }

        /// Example:
        /// ```text
        /// When account 1 creates 5 RecursiveCall contract(s)
        /// ```
        #[when(expr = "{acct} creates {int} {word} contracts")]
        fn create_contracts(
            world: &mut $world,
            owner: $crate::AccountNumber,
            n: u32,
            contract: String,
        ) {
            for _ in 0..n {
                world
                    .tester
                    .create_contract(owner, contract.clone())
                    .expect("countract creation should succeed")
            }
        }

        /// Example:
        /// ```text
        /// Then account 1 fails to create a SimpleCoin contract with 'Actor sequence invalid: 2 != 0'
        /// ```
        #[then(expr = "{acct} fails to create a {word} contract with {string}")]
        fn fail_create_contract(
            world: &mut $world,
            owner: $crate::AccountNumber,
            contract: String,
            message: String,
        ) {
            let err = world
                .tester
                .create_contract(owner, contract)
                .expect_err("contract creation should fail");
            assert!(format!("{err:?}").contains(&message))
        }

        /// Example:
        /// ```text
        /// When the seqno of account 1 is set to 2
        /// ```
        #[when(expr = "the seqno of {acct} is set to {int}")]
        fn set_seqno(world: &mut $world, acct: $crate::AccountNumber, seqno: u64) {
            // NOTE: If we called `Tester::set_account_sequence` as well then they would
            // be in sync and no error would be detected. That can be done as setup.
            world.tester.account_mut(acct).seqno = seqno;
        }

        /// Example:
        /// ```text
        /// Then the seqno of account 1 is 4
        /// ```
        #[then(expr = "the seqno of {acct} is {int}")]
        fn check_seqno(world: &mut $world, acct: $crate::AccountNumber, seqno: u64) {
            assert_eq!(world.tester.account_mut(acct).seqno, seqno)
        }
    };
}

pub type MockProvider = ethers::providers::Provider<ethers::providers::MockProvider>;
pub type TestContractCall<R> = ContractCall<MockProvider, R>;

/// Convert an FVM actor ID to `ethers` address.
pub fn id_to_h160(id: ActorID) -> ethers::core::types::Address {
    let addr = fvm_integration_tests::fevm::EthAddress::from_id(id);
    ethers::core::types::Address::from_slice(&addr.0)
}

/// Left pad a byte array to 32 bytes.
///
/// For example the _topics_ in event filtering and parsing are expected to be 32 byte words,
/// but the FVM returns 20 byte addresses to save on storage space, which need to be padded.
pub fn to_h256(bytes: &[u8]) -> H256 {
    match bytes.len() {
        32 => H256::from_slice(bytes),
        n if n < 32 => {
            let mut padded = [0u8; 32];
            padded[(32 - n)..].copy_from_slice(bytes);
            H256(padded)
        }
        _ => panic!("bytes too long for h256"),
    }
}

/// Create constructors for a smart contract, injecting a mock provider for the client,
/// because we are not going to send them to an actual blockchain.
macro_rules! contract_constructors {
    ($contract:ident) => {
        #[allow(dead_code)] // Suppress warning if this is never called.
        pub fn new_with_eth_addr(
            owner: fvm_integration_tests::fevm::EthAddress,
        ) -> $contract<$crate::MockProvider> {
            // The owner of the contract is expected to be the 160 bit hash used on Ethereum.
            let address = ethers::core::types::Address::from_slice(&owner.0);
            // A dummy client that we don't intend to use to call the contract or send transactions.
            let (client, _mock) = ethers::providers::Provider::mocked();
            $contract::new(address, std::sync::Arc::new(client))
        }

        #[allow(dead_code)] // Suppress warning if this is never called.
        pub fn new_with_actor_id(owner: fvm_shared::ActorID) -> $contract<$crate::MockProvider> {
            let owner = fvm_integration_tests::fevm::EthAddress::from_id(owner);
            new_with_eth_addr(owner)
        }
    };
}

mod simple_coin_world {
    use cucumber::gherkin::Step;
    use cucumber::{given, then, when, World};
    use ethers::types::U256;
    use evm_contracts::simple_coin::{SimpleCoin, TransferFilter};
    use fvm_shared::address::Address;

    use crate::{AccountNumber, ContractTester, MockProvider, DEFAULT_GAS};

    contract_constructors!(SimpleCoin);

    // `World` is your shared, likely mutable state.
    // Cucumber constructs it via `Default::default()` for each scenario.
    #[derive(World, Default, Debug)]
    pub struct SimpleCoinWorld {
        pub tester: ContractTester,
    }

    impl SimpleCoinWorld {
        /// Get the last deployed contract.
        fn get_contract(&self) -> (SimpleCoin<MockProvider>, Address) {
            self.tester.last_contract(new_with_actor_id)
        }

        /// Parse the events from the last send coin call.
        fn parse_transfers(&self) -> Vec<TransferFilter> {
            let (contract, contract_addr) = self.get_contract();
            self.tester.parse_events(contract_addr, |topics, data| {
                contract.decode_event("Transfer", topics, data)
            })
        }
    }

    contract_matchers!(SimpleCoinWorld);

    /// Example:
    /// ```text
    /// When account 1 sends account 2 1000 coins
    /// ```
    #[when(expr = "{acct} sends {acct} {int} coin(s)")]
    fn send_coin(
        world: &mut SimpleCoinWorld,
        sender: AccountNumber,
        receiver: AccountNumber,
        coins: u64,
    ) {
        let (contract, contract_addr) = world.get_contract();
        let receiver_addr = world.tester.account_h160(receiver);
        let call = contract.send_coin(receiver_addr, U256::from(coins));
        let _sufficient = world
            .tester
            .call_contract(sender, contract_addr, call.gas(DEFAULT_GAS))
            .expect("send_coin should succeed");
    }

    /// Example:
    /// ```text
    /// Then the balance of account 2 is 1000 coins
    /// ```
    #[then(expr = "the balance of {acct} is {int} coin(s)")]
    fn check_balance(world: &mut SimpleCoinWorld, acct: AccountNumber, coins: u64) {
        let (contract, contract_addr) = world.get_contract();
        let addr = world.tester.account_h160(acct);
        let call = contract.get_balance(addr);
        let balance = world
            .tester
            .call_contract(acct, contract_addr, call.gas(DEFAULT_GAS))
            .expect("get_balance should succeed");

        assert_eq!(balance, U256::from(coins))
    }

    /// Example:
    /// ```text
    /// a Transfer event of 4000 coins from account 1 to account 2 is emitted
    /// ```
    #[then(expr = "a Transfer event of {int} coins from {acct} to {acct} is emitted")]
    fn check_transfer_event(
        world: &mut SimpleCoinWorld,
        coins: u64,
        sender: AccountNumber,
        receiver: AccountNumber,
    ) {
        let transfers = world.parse_transfers();
        assert_eq!(transfers.len(), 1, "expected exactly 1 event");
        assert_eq!(transfers[0].from, world.tester.account_h160(sender));
        assert_eq!(transfers[0].to, world.tester.account_h160(receiver));
        assert_eq!(transfers[0].value, U256::from(coins));
    }
}

mod recursive_call_world {
    use std::collections::HashMap;
    use std::str::FromStr;

    use cucumber::gherkin::Step;
    use cucumber::{given, then, when, World};
    use ethers::types::H160;
    use evm_contracts::recursive_call::RecursiveCall;

    use crate::{AccountNumber, ContractNumber, ContractTester, DEFAULT_GAS};

    contract_constructors!(RecursiveCall);

    #[derive(World, Default, Debug)]
    pub struct RecursiveCallWorld {
        pub tester: ContractTester,
    }

    contract_matchers!(RecursiveCallWorld);

    /// Mirroring `RevertCall.Action` in Solidity.
    #[repr(u8)]
    enum Action {
        DELEGATECALL = 0u8,
        CALL,
        REVERT,
    }

    impl FromStr for Action {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "DELEGATECALL" => Ok(Action::DELEGATECALL),
                "CALL" => Ok(Action::CALL),
                "REVERT" => Ok(Action::REVERT),
                other => Err(format!("invalid Action: {other}")),
            }
        }
    }

    /// Example:
    /// ```text
    /// And account 1 calls recurse on contract 3 with max depth 3 and contract addresses:
    ///   | addresses  |
    ///   | contract 2 |
    ///   | contract 1 |
    ///   | contract 2 |
    /// ```
    #[when(expr = "{acct} calls recurse on {cntr} with max depth {int} and contract addresses:")]
    fn recurse(
        world: &mut RecursiveCallWorld,
        acct: AccountNumber,
        cntr: ContractNumber,
        max_depth: u32,
        step: &Step,
    ) {
        let (contract, contract_addr) = world.tester.contract(cntr, new_with_actor_id);

        let mut addresses = Vec::new();
        let mut actions = Vec::new();

        if let Some(table) = step.table.as_ref() {
            let header = table.rows.first().expect("expected table header");

            for row in table.rows.iter().skip(1) {
                let kvs = header
                    .iter()
                    .zip(row)
                    .filter_map(|(k, v)| {
                        if v.is_empty() {
                            None
                        } else {
                            Some((k.clone(), v.clone()))
                        }
                    })
                    .collect::<HashMap<_, _>>();

                let action = kvs
                    .get("action")
                    .map(|s| Action::from_str(s.as_str()))
                    .transpose()
                    .unwrap()
                    .unwrap_or(Action::DELEGATECALL);

                let cntr = kvs
                    .get("address")
                    .map(|s| ContractNumber::from_str(s.as_str()))
                    .transpose()
                    .unwrap()
                    .unwrap_or(ContractNumber(0));

                let contract_addr = world.tester.deployed_contract(cntr).addr_to_h160();

                actions.push(action as u8);
                addresses.push(contract_addr);
            }
        }

        let call = contract
            .recurse(addresses, actions, max_depth, 0)
            .gas(DEFAULT_GAS);

        let success = world
            .tester
            .call_contract(acct, contract_addr, call)
            .expect("recurse should not fail");

        assert!(success, "recurse should return success");
    }

    /// Example:
    /// ```text
    /// Then the state of depth and sender of the contracts are:
    ///   | contract   | depth | sender    |
    ///   | contract 2 | 1     | account 1 |
    ///   | contract 1 | 0     |           |
    /// ```
    #[then(expr = "the state of depth and sender of the contracts are:")]
    fn check_state(world: &mut RecursiveCallWorld, step: &Step) {
        if let Some(table) = step.table.as_ref() {
            // Use some existing account to probe state.
            let acct = AccountNumber(0);
            // NOTE: skip header
            for row in table.rows.iter().skip(1) {
                let cntr = ContractNumber::from_str(&row[0]).expect("not a contract number");
                let (contract, addr) = world.tester.contract(cntr, new_with_actor_id);

                if !row[1].is_empty() {
                    let call = contract.depth().gas(DEFAULT_GAS);
                    let exp_depth = u32::from_str(&row[1]).expect("not a depth");
                    let depth = world
                        .tester
                        .call_contract(acct, addr, call)
                        .expect("depth should not fail");

                    assert_eq!(depth, exp_depth, "depth of {cntr}");
                };

                let sender = if row[2].is_empty() {
                    None
                } else if let Ok(acct) = AccountNumber::from_str(&row[2]) {
                    Some(world.tester.account_h160(acct))
                } else if let Ok(cntr) = ContractNumber::from_str(&row[2]) {
                    // NOTE: We are not using the ActorID here.
                    let delegated_addr = world.tester.deployed_contract(cntr).eth_address;
                    Some(H160::from_slice(&delegated_addr.0))
                } else if let Ok(bytes) = hex::decode(row[2].strip_prefix("0x").unwrap_or(&row[2]))
                {
                    Some(H160::from_slice(&bytes))
                } else {
                    panic!("unexpected sender: {}", row[2]);
                };

                if let Some(exp_sender) = sender {
                    let call = contract.sender().gas(DEFAULT_GAS);
                    let sender = world
                        .tester
                        .call_contract(acct, addr, call)
                        .expect("sender should not fail");

                    assert_eq!(sender, exp_sender, "sender of {cntr}");
                }
            }
        }
    }
}

mod bank_account {
    use cucumber::gherkin::Step;
    use cucumber::{given, then, when, World};
    use ethers::types::H160;
    use fvm_integration_tests::fevm::{EthAddress, EAM_ACTOR_ID};
    use fvm_shared::address::Address;

    use crate::{AccountNumber, ContractTester, DEFAULT_GAS};

    mod bank {
        use evm_contracts::bank::Bank;

        contract_constructors!(Bank);
    }
    mod account {
        use evm_contracts::account::Account;

        contract_constructors!(Account);
    }

    #[derive(World, Default, Debug)]
    pub struct BankAccountWorld {
        pub tester: ContractTester,
        pub bank_accounts: Vec<H160>,
    }

    contract_matchers!(BankAccountWorld);

    impl BankAccountWorld {
        /// Get the Ethereum address of the bank contract (assumed to be the last deployed contract).
        fn bank_eth_addr(&self) -> EthAddress {
            self.tester
                .contracts
                .last()
                .expect("no contracts deployed yet")
                .eth_address
        }
        /// Get the FVM Address address of the last opened bank account.
        fn last_bank_account_addr(&self) -> Address {
            let bank_account_eth_addr = self.bank_accounts.last().expect("no bank accounts yet");
            let f4_addr =
                Address::new_delegated(EAM_ACTOR_ID.id().unwrap(), &bank_account_eth_addr.0)
                    .unwrap();
            f4_addr
        }
    }

    #[when(expr = "{acct} opens a bank account")]
    fn open_bank_account(world: &mut BankAccountWorld, acct: AccountNumber) {
        let (contract, contract_addr) = world.tester.last_contract(bank::new_with_actor_id);
        let call = contract.open_account().gas(DEFAULT_GAS);

        let bank_account_address = world
            .tester
            .call_contract(acct, contract_addr, call)
            .expect("open_account should work");

        world.bank_accounts.push(bank_account_address)
    }

    #[then(expr = "the owner of the bank is {acct}")]
    fn check_bank_owner(world: &mut BankAccountWorld, acct: AccountNumber) {
        let (contract, contract_addr) = world.tester.last_contract(bank::new_with_actor_id);
        let call = contract.owner().gas(DEFAULT_GAS);

        let owner = world
            .tester
            .call_contract(acct, contract_addr, call)
            .expect("bank owner should work");

        assert_eq!(owner, world.tester.account_h160(acct))
    }

    #[then(expr = "the owner of the bank account is {acct}")]
    fn check_account_owner(world: &mut BankAccountWorld, acct: AccountNumber) {
        let bank_eth_addr = world.bank_eth_addr();
        let contract_addr = world.last_bank_account_addr();
        let contract = account::new_with_eth_addr(bank_eth_addr);
        let call = contract.owner().gas(DEFAULT_GAS);

        let owner = world
            .tester
            .call_contract(acct, contract_addr, call)
            .expect("account owner should work");

        assert_eq!(owner, world.tester.account_h160(acct))
    }

    #[then(expr = "the bank of the bank account is set")]
    fn check_account_bank(world: &mut BankAccountWorld) {
        let bank_eth_addr = world.bank_eth_addr();
        let contract_addr = world.last_bank_account_addr();
        let contract = account::new_with_eth_addr(bank_eth_addr);
        let call = contract.bank().gas(DEFAULT_GAS);

        let bank = world
            .tester
            .call_contract(AccountNumber(0), contract_addr, call)
            .expect("account bank should work");

        assert_eq!(bank.0, bank_eth_addr.0)
    }
}
