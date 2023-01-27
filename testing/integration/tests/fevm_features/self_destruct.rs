use std::str::FromStr;

use cucumber::gherkin::Step;
use cucumber::{given, then, when, World};
use ethers::types::H160;
use fvm_integration_tests::fevm::EthAddress;
use fvm_shared::address::Address;

use crate::common::*;

mod self_destruct_on_create {
    use evm_contracts::self_destruct::SelfDestructOnCreate;
    crate::contract_constructors!(SelfDestructOnCreate);
}

mod self_destruct_chain {
    use evm_contracts::self_destruct::SelfDestructChain;
    crate::contract_constructors!(SelfDestructChain);
}

mod metamorphic_contract_factory {
    use evm_contracts::metamorphic::MetamorphicContractFactory;
    crate::contract_constructors!(MetamorphicContractFactory);
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

#[when(expr = "the code of transient contract {contract_name} is loaded")]
fn load_transient_code(world: &mut SelfDestructWorld, contract_name: ContractName) {
    let code = world.tester.get_contract_code(&contract_name);
    world.tester.set_next_constructor_args(code)
}

#[when(
    expr = "{acct} calls deployMetamorphicContractWithConstructor on {cntr} with the code of {contract_name}"
)]
fn deploy_metamorph(
    world: &mut SelfDestructWorld,
    acct: AccountNumber,
    cntr: ContractNumber,
    contract_name: ContractName,
) {
    let code = world.tester.get_contract_code(&contract_name);
    let code = ethers::types::Bytes::from(code);
    let (contract, contract_addr) = world
        .tester
        .contract(cntr, metamorphic_contract_factory::new_with_actor_id);

    // As per `containsCaller` in `Metamorphic.sol` the first 20 bytes of the salt must match the caller. T
    // To deploy more alternatives we'd need to use different salts; here I just leave it on 0.
    let account_addr = world.tester.account_h160(acct);
    let mut salt = [0u8; 32];
    salt[..20].copy_from_slice(&account_addr.0);

    let call = contract.deploy_metamorphic_contract_with_constructor(salt, code);

    let metamorphic_addr = world
        .tester
        .call_contract(acct, contract_addr, call)
        .expect("deploy should succeed");

    eprintln!("metamorphic_addr = {}", hex::encode(metamorphic_addr.0));

    // Look up what actor it is.
    let f410_addr = h160_to_f410(&metamorphic_addr);
    let actor_id = world
        .tester
        .actor_id(&f410_addr)
        .expect("metamorphic contract should exist as an actor");

    // Put it among the deployed contracts, so we can refer to is balance in the matchers.
    let deployed = DeployedContract {
        _name: contract_name,
        owner: world.tester.account(acct).account,
        address: Address::new_id(actor_id),
        eth_address: EthAddress(metamorphic_addr.0),
    };
    world.tester.contracts.push(deployed)
}
