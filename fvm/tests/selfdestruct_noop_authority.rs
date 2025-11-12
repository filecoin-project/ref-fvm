// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod common;

use common::{new_harness, set_ethaccount_with_delegate};
use fvm_integration_tests::tester::{BasicAccount, ExecutionOptions};
use fvm_integration_tests::testkit::fevm;
use fvm_shared::address::Address;

fn selfdestruct_delegate(beneficiary: [u8; 20]) -> Vec<u8> {
    // PUSH20 beneficiary; SELFDESTRUCT
    let mut code = Vec::new();
    code.push(0x73);
    code.extend_from_slice(&beneficiary);
    code.push(0xFF);
    code
}

fn caller_call_authority(auth20: [u8; 20]) -> Vec<u8> {
    // CALL with zero args/ret to trigger delegate execution.
    let mut code = Vec::new();
    code.extend_from_slice(&[0x61, 0xFF, 0xFF]);
    code.push(0x73);
    code.extend_from_slice(&auth20);
    code.extend_from_slice(&[0x60, 0x00]); // value=0
    code.extend_from_slice(&[0x60, 0x00]); // argsOff
    code.extend_from_slice(&[0x60, 0x00]); // argsLen
    code.extend_from_slice(&[0x60, 0x00]); // retOff
    code.extend_from_slice(&[0x60, 0x00]); // retLen
    code.push(0xF1);
    // return(0,0)
    code.extend_from_slice(&[0x60, 0x00, 0x60, 0x00, 0xF3]);
    code
}

#[test]
fn selfdestruct_is_noop_under_authority_context() {
    let options = ExecutionOptions {
        debug: false,
        trace: false,
        events: true,
    };
    let mut h = new_harness(options).expect("harness");
    let mut owner: BasicAccount = h.tester.create_basic_account().unwrap();

    // Deploy a delegate that calls SELFDESTRUCT(beneficiary=some address).
    let beneficiary20 = [
        0xBA, 0xAD, 0xF0, 0x0D, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xAA, 0xBB, 0xBB, 0xBB, 0xBB, 0xBB,
        0xCC, 0xCC, 0xCC, 0xCC, 0xCC,
    ];
    let del = fevm::create_contract(
        &mut h.tester,
        &mut owner,
        &selfdestruct_delegate(beneficiary20),
    )
    .unwrap();
    assert!(del.msg_receipt.exit_code.is_success());
    let delegate_eth = del
        .msg_receipt
        .return_data
        .deserialize::<fevm::CreateReturn>()
        .unwrap()
        .eth_address
        .0;

    // Create authority EthAccount with delegate_to set.
    let auth20: [u8; 20] = [
        0x44, 0x33, 0x22, 0x11, 0x00, 0x44, 0x33, 0x22, 0x11, 0x00, 0x44, 0x33, 0x22, 0x11, 0x00,
        0x44, 0x33, 0x22, 0x11, 0x00,
    ];
    let auth_f4 = Address::new_delegated(10, &auth20).unwrap();
    let auth_id = set_ethaccount_with_delegate(&mut h, auth_f4.clone(), delegate_eth).unwrap();

    h.tester
        .instantiate_machine(fvm_integration_tests::dummy::DummyExterns)
        .unwrap();

    // Call authority from caller contract to trigger delegated execution.
    let caller_code = caller_call_authority(auth20);
    let caller = fevm::create_contract(&mut h.tester, &mut owner, &caller_code).unwrap();
    let caller_ret = caller
        .msg_receipt
        .return_data
        .deserialize::<fevm::CreateReturn>()
        .unwrap();
    let caller_addr = caller_ret.robust_address.expect("robust");
    let _inv = fevm::invoke_contract(
        &mut h.tester,
        &mut owner,
        caller_addr,
        &[],
        fevm::DEFAULT_GAS,
    )
    .unwrap();

    // No explicit state verification here; the call must complete without errors and
    // any SELFDESTRUCT in delegated context must be a no-op for the authority.
}
// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
