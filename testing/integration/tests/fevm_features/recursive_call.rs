use std::collections::HashMap;
use std::str::FromStr;

use cucumber::gherkin::Step;
use cucumber::{given, then, when, World};
use ethers::types::H160;
use evm_contracts::recursive_call::RecursiveCall;

use crate::common::{AccountNumber, ContractNumber, ContractTester, DEFAULT_GAS};

crate::contract_constructors!(RecursiveCall);

#[derive(World, Debug)]
pub struct RecursiveCallWorld {
    pub tester: ContractTester,
}

impl Default for RecursiveCallWorld {
    fn default() -> Self {
        Self {
            tester: ContractTester::new_with_default_versions("RecursiveCall"),
        }
    }
}

crate::contract_matchers!(RecursiveCallWorld);

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
            } else if let Ok(bytes) = hex::decode(row[2].strip_prefix("0x").unwrap_or(&row[2])) {
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
