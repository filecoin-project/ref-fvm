use cucumber::gherkin::Step;
use cucumber::{given, then, when, World};

use crate::common::{AccountNumber, ContractTester};

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
