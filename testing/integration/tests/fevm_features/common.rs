// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use std::fmt::Display;
use std::str::FromStr;

use cucumber::gherkin::Step;
use cucumber::Parameter;
use ethers::abi::{Detokenize, Tokenize};
use ethers::prelude::builders::ContractCall;
use ethers::prelude::{decode_function_data, AbiError};
use ethers::types::{Bytes, H160, H256};
use fvm::executor::ApplyFailure;
use fvm::machine::{DefaultMachine, Machine};
use fvm::state_tree::{ActorState, StateTree};
use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::tester::{
    Account as TestAccount, BasicAccount, BasicTester, INITIAL_ACCOUNT_BALANCE,
};
use fvm_integration_tests::testkit::fevm::{self, CreateReturn, EthAddress, EAM_ADDRESS};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::BytesDe;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::event::StampedEvent;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use fvm_shared::ActorID;
use lazy_static::__Deref;
use libsecp256k1::SecretKey;

use crate::CONTRACTS;

mod bundles {
    include!("../bundles/mod.rs");
}

/// Get a contract from the pre-loaded sources.
pub fn get_contract_code<'a>(sol_name: &'a str, contract_name: &'a str) -> &'a [u8] {
    CONTRACTS
        .get(&(sol_name, contract_name))
        .ok_or_else(|| format!("contract {sol_name}/{contract_name} hasn't been loaded"))
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

/// Hexadecimal bytes of length 20.
#[derive(Parameter, Debug, Clone)]
#[param(name = "hex160", regex = r"0x([a-fA-F0-9]{40})")]
pub struct Hex160(pub H160);

impl FromStr for Hex160 {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match hex::decode(s.strip_prefix("0x").unwrap_or(s)) {
            Ok(bs) => Ok(Self(H160::from_slice(&bs))),
            Err(e) => Err(format!("not hex bytes: {s}; {e}")),
        }
    }
}

impl Display for Hex160 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

/// Tokens in Atto.
#[derive(Parameter, Debug, Clone)]
#[param(name = "atto", regex = r"(\d+) atto")]
pub struct Atto(pub TokenAmount);

impl FromStr for Atto {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match u64::from_str(s.strip_suffix(" atto").unwrap_or(s)) {
            Ok(n) => Ok(Self(TokenAmount::from_atto(n))),
            Err(_) => Err(format!("not an integer: {s}")),
        }
    }
}

impl Display for Atto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} atto", self.0.atto())
    }
}

/// Contract name can either be a simple name, in which case the tester will load it from
/// the sol file it's been instantiated with, or it supply its own solidity file name,
/// so that we can load contracts from multiple solidity files into the same test.
///
/// # Example
/// ```text
/// When account 1 creates a Metamorphic / TransientContract contract
/// And account 1 creates a Cocoon contract
/// ```
#[derive(Parameter, Debug, Clone)]
#[param(
    name = "contract_name",
    regex = r"(([A-Za-z0-9_]+(( / )|/))?[A-Za-z0-9_]+)"
)]
pub struct ContractName {
    pub sol_name: Option<String>,
    pub contract_name: String,
}

impl FromStr for ContractName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s
            .split('/')
            .into_iter()
            .map(|p| p.trim())
            .collect::<Vec<_>>()
            .as_slice()
        {
            [cn] => Ok(Self {
                sol_name: None,
                contract_name: (*cn).to_owned(),
            }),
            [sn, cn] => Ok(Self {
                sol_name: Some((*sn).to_owned()),
                contract_name: (*cn).to_owned(),
            }),
            _ => Err(format!(
                "expected 'file / contract' or just 'contract'; got {s}"
            )),
        }
    }
}

impl Display for ContractName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(sol_name) = &self.sol_name {
            write!(f, "{} / ", sol_name)?;
        }
        write!(f, "{}", self.contract_name)
    }
}

/// Remember what contract was deployed.
#[derive(Debug, Clone)]
pub struct DeployedContract {
    /// Name would be useful if we had multiple contracts in the same solidity file
    /// and wanted to check what contract was deployed at a certain slot.
    pub _name: ContractName,
    pub owner: TestAccount,
    /// The ActorID address.
    pub address: Address,
    /// The ethereum address from `CreateReturn`, produced by the EAM actor.
    pub eth_address: EthAddress,
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
#[derive(Debug, Clone)]
pub struct ExecError {
    pub exit_code: ExitCode,
    pub failure_info: Option<ApplyFailure>,
}

/// Common machinery for all worlds to created and call contracts.
pub struct ContractTester {
    pub tester: BasicTester,
    /// Name of the solidity file we are adding contracts from.
    pub sol_name: &'static str,
    /// Accounts created by the tester.
    pub accounts: Vec<BasicAccount>,
    /// Contracts created by the tester; `(owner, contract_address)`.
    pub contracts: Vec<DeployedContract>,
    /// Events emitted by the last contract invocation.
    pub last_events: Vec<StampedEvent>,
    /// Any potential error with the last execution.
    pub last_exec_error: Option<ExecError>,
    /// Any constructor arguments we want to use with the next contract creation,
    /// after which it is cleared. It is expected to be in ABI encoded format.
    pub next_constructor_args: Option<Vec<u8>>,
    /// Any tokens to send to the next contract invocation.
    pub next_token_amount: Option<TokenAmount>,
}

impl std::fmt::Debug for ContractTester {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContractTester")
            .field("accounts", &self.accounts)
            .field("contracts", &self.contracts)
            .field("last_events", &self.last_events)
            .finish()
    }
}

impl ContractTester {
    pub fn new_with_default_versions(sol_name: &'static str) -> Self {
        Self::new(NetworkVersion::V18, StateTreeVersion::V5, sol_name)
    }

    pub fn new(nv: NetworkVersion, stv: StateTreeVersion, sol_name: &'static str) -> Self {
        let blockstore = MemoryBlockstore::default();
        let tester = match bundles::new_tester(nv, stv, blockstore) {
            Ok(t) => t,
            Err(e) => panic!("error creating tester with NV={nv} and STV={stv:?}: {e}"),
        };
        Self {
            tester,
            sol_name,
            accounts: Vec::new(),
            contracts: Vec::new(),
            last_events: Vec::new(),
            last_exec_error: None,
            next_constructor_args: None,
            next_token_amount: None,
        }
    }

    /// Read the raw contract code. The returned value can be passed to smart contract methods.
    pub fn get_contract_code(&self, contract: &ContractName) -> ethers::abi::ethabi::Bytes {
        let code = get_contract_code(
            contract.sol_name.as_deref().unwrap_or(self.sol_name),
            &contract.contract_name,
        );
        ethers::abi::ethabi::Bytes::from(code)
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

            let account = BasicAccount {
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

                let account = BasicAccount { account, seqno: 0 };

                self.accounts.push(account);
            }
        }
    }

    /// Get a mutable reference to an account.
    pub fn account_mut(&mut self, acct: AccountNumber) -> &mut BasicAccount {
        self.accounts
            .get_mut(acct.0)
            .ok_or_else(|| format!("{acct} has not been created"))
            .unwrap()
    }

    /// Get a reference to a created account.
    pub fn account(&self, acct: AccountNumber) -> &BasicAccount {
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

    /// Get the state tree to
    fn state_tree(
        &mut self,
    ) -> &StateTree<<DefaultMachine<MemoryBlockstore, DummyExterns> as Machine>::Blockstore> {
        self.ensure_machine_instantiated();
        let executor = self.tester.executor.as_ref().expect("machine instantiated");
        let machine = executor.deref();
        machine.state_tree()
    }

    /// Look up the actor ID by address.
    pub fn actor_id(&mut self, addr: &Address) -> Option<ActorID> {
        self.state_tree()
            .lookup_id(addr)
            .expect("actor ID lookup should succeed")
    }

    /// Get the state of an actor, if it exists.
    pub fn actor_state(&mut self, addr: &Address) -> Option<ActorState> {
        self.state_tree()
            .get_actor_by_address(addr)
            .expect("actor lookup should succeed")
    }

    /// An f410 account is one managed by the EAM actor.
    pub fn f410_account_state(&mut self, account: &H160) -> Option<ActorState> {
        let addr = h160_to_f410(account);

        self.actor_state(&addr)
    }

    /// ABI encode some constructor arguments for the next contract creation.
    ///
    /// When they have multiple arguments, pass them as a tuple.
    pub fn set_next_constructor_args<T: Tokenize>(&mut self, args: T) {
        let bytes = ethers::abi::encode(&args.into_tokens());
        self.next_constructor_args = Some(bytes);
    }

    /// Deploy a contract owned by an account.
    pub fn create_contract(
        &mut self,
        owner: AccountNumber,
        contract_name: ContractName,
    ) -> Result<(), ExecError> {
        self.ensure_machine_instantiated();

        // Need to clone because I have to pass 2 mutable references to `fevm::create_contract`.
        let mut account = self.account_mut(owner).clone();
        let creator = account.account;
        let contract = get_contract_code(
            contract_name.sol_name.as_deref().unwrap_or(self.sol_name),
            &contract_name.contract_name,
        );

        let value = self.next_token_amount.take().unwrap_or_default();

        let create_res = if let Some(args) = self.next_constructor_args.take() {
            let initcode = [contract, &args].concat();
            fevm::create_contract(&mut self.tester, &mut account, &initcode, value)
        } else {
            fevm::create_contract(&mut self.tester, &mut account, contract, value)
        };

        let create_res = create_res.expect("error creating contract");

        *self.account_mut(owner) = account;

        if !create_res.msg_receipt.exit_code.is_success() {
            let err = ExecError {
                exit_code: create_res.msg_receipt.exit_code,
                failure_info: create_res.failure_info,
            };
            self.last_exec_error = Some(err.clone());
            return Err(err);
        } else {
            self.last_exec_error = None;
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

        let gas = call
            .tx
            .gas()
            .map(|g| g.as_u64().try_into().expect("too much gas"))
            .unwrap_or(DEFAULT_GAS);

        // `next_token_amount` takes precedence so we can say that after the next call it's always empty.
        let value = self
            .next_token_amount
            .take()
            .or_else(|| call.tx.value().map(|v| TokenAmount::from_atto(v.as_u64())))
            .unwrap_or_default();

        let invoke_res = fevm::invoke_contract(
            &mut self.tester,
            &mut account,
            contract_addr,
            &input,
            gas,
            value,
        );

        let invoke_res = invoke_res.expect("error invoking contract");

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
            let err = ExecError {
                exit_code: invoke_res.msg_receipt.exit_code,
                failure_info: invoke_res.failure_info,
            };
            self.last_exec_error = Some(err.clone());
            return Err(err);
        } else {
            self.last_exec_error = None;
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
/// use crate::common::*;
/// ```
#[macro_export]
macro_rules! contract_matchers {
    ($world:ident) => {
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

        #[given(expr = "a non-existing f410 account {hex160}")]
        #[then(expr = "f410 account {hex160} does not exist")]
        fn check_f410_exists_not(world: &mut $world, account: Hex160) {
            assert!(world.tester.f410_account_state(&account.0).is_none())
        }

        #[then(expr = "f410 account {hex160} exists")]
        fn check_f410_exists(world: &mut $world, account: Hex160) {
            assert!(world.tester.f410_account_state(&account.0).is_some())
        }

        #[then(expr = "the balance of f410 account {hex160} is {atto}")]
        fn check_f410_balance(world: &mut $world, account: Hex160, atto: Atto) {
            let state = world
                .tester
                .f410_account_state(&account.0)
                .expect("f410 account exists");

            assert_eq!(state.balance, atto.0)
        }

        #[then(expr = "the balance of {acct} is {atto}")]
        fn check_acct_balance(world: &mut $world, acct: AccountNumber, atto: Atto) {
            let id = world.tester.account_id(acct);
            let addr = fvm_shared::address::Address::new_id(id);
            let state = world.tester.actor_state(&addr).expect("account exists");

            assert_eq!(state.balance, atto.0)
        }

        #[then(expr = "the balance of {cntr} is {atto}")]
        fn check_cntr_balance(world: &mut $world, cntr: ContractNumber, atto: Atto) {
            let addr = world.tester.deployed_contract(cntr).address;
            let state = world.tester.actor_state(&addr).expect("contract exists");

            assert_eq!(state.balance, atto.0)
        }

        #[when(expr = "{acct} creates a {contract_name} contract")]
        fn create_contract(world: &mut $world, owner: AccountNumber, contract: ContractName) {
            world
                .tester
                .create_contract(owner, contract)
                .expect("countract creation should succeed")
        }

        #[when(expr = "the value sent to the contract is {atto}")]
        fn set_next_token_amount(world: &mut $world, atto: Atto) {
            world.tester.next_token_amount = Some(atto.0)
        }

        #[when(expr = "{acct} creates {int} {contract_name} contract(s)")]
        fn create_contracts(
            world: &mut $world,
            owner: AccountNumber,
            n: u32,
            contract: ContractName,
        ) {
            let next_constructor_args = world.tester.next_constructor_args.take();
            let next_token_amount = world.tester.next_token_amount.take();
            for _ in 0..n {
                world.tester.next_constructor_args = next_constructor_args.clone();
                world.tester.next_token_amount = next_token_amount.clone();
                world
                    .tester
                    .create_contract(owner, contract.clone())
                    .expect("countract creation should succeed");
            }
        }

        #[when(expr = "{acct} tries to create a {contract_name} contract")]
        fn try_create_contract(world: &mut $world, owner: AccountNumber, contract: ContractName) {
            let _ = world.tester.create_contract(owner, contract);
        }

        #[then(expr = "the execution fails with message {string}")]
        fn check_last_exec_error(world: &mut $world, message: String) {
            let err = world
                .tester
                .last_exec_error
                .as_ref()
                .expect("last exec should have failed");

            assert!(
                format!("{err:?}").contains(&message),
                "expected {message}, got {err:?}"
            )
        }

        #[when(expr = "the seqno of {acct} is set to {int}")]
        fn set_seqno(world: &mut $world, acct: AccountNumber, seqno: u64) {
            // NOTE: If we called `Tester::set_account_sequence` as well then they would
            // be in sync and no error would be detected. That can be done as setup.
            world.tester.account_mut(acct).seqno = seqno;
        }

        #[then(expr = "the seqno of {acct} is {int}")]
        fn check_seqno(world: &mut $world, acct: AccountNumber, seqno: u64) {
            assert_eq!(world.tester.account_mut(acct).seqno, seqno)
        }
    };
}

pub type MockProvider = ethers::providers::Provider<ethers::providers::MockProvider>;
pub type TestContractCall<R> = ContractCall<MockProvider, R>;

/// Convert an FVM actor ID to `ethers` address.
pub fn id_to_h160(id: ActorID) -> ethers::core::types::Address {
    let addr = fevm::EthAddress::from_id(id);
    ethers::core::types::Address::from_slice(&addr.0)
}

/// Convert an Ethereum adress to a delegated address
pub fn h160_to_f410(addr: &H160) -> Address {
    Address::new_delegated(EAM_ADDRESS.id().unwrap(), &addr.0).expect("delegated address")
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
#[macro_export]
macro_rules! contract_constructors {
    ($contract:ident) => {
        #[allow(dead_code)] // Suppress warning if this is never called.
        pub fn new_with_eth_addr(
            owner: fvm_integration_tests::testkit::fevm::EthAddress,
        ) -> $contract<$crate::common::MockProvider> {
            // The owner of the contract is expected to be the 160 bit hash used on Ethereum.
            let address = ethers::core::types::Address::from_slice(&owner.0);
            // A dummy client that we don't intend to use to call the contract or send transactions.
            let (client, _mock) = ethers::providers::Provider::mocked();
            $contract::new(address, std::sync::Arc::new(client))
        }

        #[allow(dead_code)] // Suppress warning if this is never called.
        pub fn new_with_actor_id(
            owner: fvm_shared::ActorID,
        ) -> $contract<$crate::common::MockProvider> {
            let owner = fvm_integration_tests::testkit::fevm::EthAddress::from_id(owner);
            new_with_eth_addr(owner)
        }
    };
}
