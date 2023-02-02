// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cucumber::gherkin::Step;
use cucumber::{given, then, when, World};
use ethers::types::H160;
use fvm_integration_tests::testkit::fevm::EthAddress;

use crate::common::*;

mod bank {
    use evm_contracts::bank_account::Bank;

    crate::contract_constructors!(Bank);
}
mod account {
    use evm_contracts::bank_account::Account;

    crate::contract_constructors!(Account);
}

#[derive(World, Debug)]
pub struct BankAccountWorld {
    pub tester: ContractTester,
    pub bank_accounts: Vec<H160>,
}

impl Default for BankAccountWorld {
    fn default() -> Self {
        Self {
            tester: ContractTester::new_with_default_versions("BankAccount"),
            bank_accounts: Vec::new(),
        }
    }
}

crate::contract_matchers!(BankAccountWorld);

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
    fn last_bank_account_eth_addr(&self) -> EthAddress {
        let bank_account_eth_addr = self.bank_accounts.last().expect("no bank accounts yet");
        EthAddress(bank_account_eth_addr.0)
    }
}

#[when(expr = "{acct} opens a bank account")]
fn open_bank_account(world: &mut BankAccountWorld, acct: AccountNumber) {
    let contract = world.tester.last_contract(bank::new_with_eth_addr);
    let call = contract.open_account();

    let bank_account_address = world
        .tester
        .call_contract(acct, call)
        .expect("open_account should work");

    world.bank_accounts.push(bank_account_address)
}

#[then(expr = "the owner of the bank is {acct}")]
fn check_bank_owner(world: &mut BankAccountWorld, acct: AccountNumber) {
    let contract = world.tester.last_contract(bank::new_with_eth_addr);
    let call = contract.owner();

    let owner = world
        .tester
        .call_contract(acct, call)
        .expect("bank owner should work");

    assert_eq!(owner, world.tester.account_h160(acct))
}

#[then(expr = "the owner of the bank account is {acct}")]
fn check_account_owner(world: &mut BankAccountWorld, acct: AccountNumber) {
    let contract_addr = world.last_bank_account_eth_addr();
    let contract = account::new_with_eth_addr(contract_addr);
    let call = contract.owner();

    let owner = world
        .tester
        .call_contract(acct, call)
        .expect("account owner should work");

    assert_eq!(owner, world.tester.account_h160(acct))
}

#[then(expr = "the bank of the bank account is set")]
fn check_account_bank(world: &mut BankAccountWorld) {
    let contract_addr = world.last_bank_account_eth_addr();
    let contract = account::new_with_eth_addr(contract_addr);
    let call = contract.bank();

    let bank = world
        .tester
        .call_contract(AccountNumber(0), call)
        .expect("account bank should work");

    assert_eq!(bank.0, world.bank_eth_addr().0)
}
