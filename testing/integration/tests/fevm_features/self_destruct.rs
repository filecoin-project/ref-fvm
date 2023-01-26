use cucumber::gherkin::Step;
use cucumber::{given, then, when, World};
use ethers::types::H160;

use crate::common::*;

mod self_destruct_on_create {
    use evm_contracts::self_destruct::SelfDestructOnCreate;

    crate::contract_constructors!(SelfDestructOnCreate);
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
