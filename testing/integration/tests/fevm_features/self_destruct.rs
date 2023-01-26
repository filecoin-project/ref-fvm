use std::str::FromStr;

use cucumber::gherkin::Step;
use cucumber::{given, then, when, World};
use ethers::types::H160;

use crate::common::*;

mod self_destruct_on_create {
    use evm_contracts::self_destruct::SelfDestructOnCreate;

    crate::contract_constructors!(SelfDestructOnCreate);
}

mod self_destruct_chain {
    use evm_contracts::self_destruct::SelfDestructChain;

    crate::contract_constructors!(SelfDestructChain);
}

#[derive(World, Debug)]
pub struct SelfDestructWorld {
    pub tester: ContractTester,
}

impl Default for SelfDestructWorld {
    fn default() -> Self {
        Self {
            tester: ContractTester::new_with_default_versions("SelfDestruct"),
        }
    }
}

crate::contract_matchers!(SelfDestructWorld);

#[when(expr = "the beneficiary is self")]
fn set_beneficiary_self(world: &mut SelfDestructWorld) {
    // Setting it to 0x00000...00 should result in the contract trying to reimburse to itself.
    let beneficiary = H160::default();
    world.tester.set_next_constructor_args(beneficiary);
}

#[when(expr = "the beneficiary is {hex160}")]
fn set_beneficiary(world: &mut SelfDestructWorld, beneficiary: Hex160) {
    world.tester.set_next_constructor_args(beneficiary.0);
}

#[when(expr = "{acct} calls destroy on {cntr} with addresses:")]
fn destroy(world: &mut SelfDestructWorld, acct: AccountNumber, cntr: ContractNumber, step: &Step) {
    let (contract, contract_addr) = world
        .tester
        .contract(cntr, self_destruct_chain::new_with_actor_id);

    let mut addresses = Vec::new();

    if let Some(table) = step.table.as_ref() {
        for row in table.rows.iter().skip(1) {
            let cntr = ContractNumber::from_str(&row[0]).expect("should be a contract number");
            let contract_addr = world.tester.deployed_contract(cntr).addr_to_h160();
            addresses.push(contract_addr);
        }
    }

    let call = contract.destroy(addresses, 0);

    world
        .tester
        .call_contract(acct, contract_addr, call)
        .expect("destroy should not fail");
}
