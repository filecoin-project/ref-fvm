use cucumber::gherkin::Step;
use cucumber::{given, then, when, World};
use ethers::types::H160;

use crate::common::{AccountNumber, ContractTester, Hex};

mod self_destruct_on_create {
    use evm_contracts::self_destruct::SelfDestructOnCreate;

    crate::contract_constructors!(SelfDestructOnCreate);
}

#[derive(World, Debug)]
pub struct SelfDestructWorld {
    pub tester: ContractTester,
    pub beneficiary: Option<H160>,
}

impl Default for SelfDestructWorld {
    fn default() -> Self {
        Self {
            tester: ContractTester::new_with_default_versions("SelfDestruct"),
            beneficiary: None,
        }
    }
}

crate::contract_matchers!(SelfDestructWorld);

#[when(expr = "the beneficiary is {hex}")]
fn set_beneficiary(world: &mut SelfDestructWorld, beneficiary: Hex) {
    let beneficiary = H160::from_slice(&beneficiary.0);
    world.beneficiary = Some(beneficiary)
}
