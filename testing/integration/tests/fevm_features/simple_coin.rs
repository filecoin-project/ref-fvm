// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cucumber::gherkin::Step;
use cucumber::{given, then, when, World};
use ethers::types::U256;
use evm_contracts::simple_coin::{SimpleCoin, TransferFilter};
use fvm_shared::address::Address;

use crate::common::*;

crate::contract_constructors!(SimpleCoin);

// `World` is your shared, likely mutable state.
// Cucumber constructs it via `Default::default()` for each scenario.
#[derive(World, Debug)]
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

impl Default for SimpleCoinWorld {
    fn default() -> Self {
        Self {
            tester: ContractTester::new_with_default_versions("SimpleCoin"),
        }
    }
}

crate::contract_matchers!(SimpleCoinWorld);

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
        .call_contract(sender, contract_addr, call)
        .expect("send_coin should succeed");
}

#[then(expr = "the balance of {acct} is {int} coin(s)")]
fn check_balance(world: &mut SimpleCoinWorld, acct: AccountNumber, coins: u64) {
    let (contract, contract_addr) = world.get_contract();
    let addr = world.tester.account_h160(acct);
    let call = contract.get_balance(addr);
    let balance = world
        .tester
        .call_contract(acct, contract_addr, call)
        .expect("get_balance should succeed");

    assert_eq!(balance, U256::from(coins))
}

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
