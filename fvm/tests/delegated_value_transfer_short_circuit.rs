mod common;

use common::{new_harness, set_ethaccount_with_delegate, install_evm_contract_at};
use fvm_integration_tests::tester::{BasicAccount, ExecutionOptions};
use fvm_integration_tests::testkit::fevm;
use fvm_shared::address::Address;
use fvm_shared::error::ExitCode;

fn make_caller_value_call(authority20: [u8; 20], value: u8, ret_len: u8) -> Vec<u8> {
    // CALL with a non-zero value, expecting transfer to fail due to insufficient funds on caller.
    let mut code = Vec::new();
    code.extend_from_slice(&[0x61, 0xFF, 0xFF]); // gas
    code.push(0x73); // address
    code.extend_from_slice(&authority20);
    code.extend_from_slice(&[0x60, value]); // non-zero value
    code.extend_from_slice(&[0x60, 0x00]); // argsOffset = 0
    code.extend_from_slice(&[0x60, 0x00]); // argsLength = 0
    code.extend_from_slice(&[0x60, 0x00]); // retOffset = 0
    code.extend_from_slice(&[0x60, ret_len]); // retLength
    code.push(0xF1); // CALL
    // Return whatever may have been written (expected none on failure)
    code.extend_from_slice(&[0x60, ret_len, 0x60, 0x00, 0xF3]);
    code
}

#[test]
#[ignore]
fn delegated_value_transfer_short_circuit() {
    let options = ExecutionOptions { debug: false, trace: false, events: true };
    let mut h = new_harness(options).expect("harness");
    let mut owner: BasicAccount = h.tester.create_basic_account().unwrap();

    // Deploy a do-nothing delegate.
    let delegate_eth: [u8;20] = [
        0x21,0x22,0x23,0x24,0x25,0x26,0x27,0x28,0x29,0x2A,
        0x2B,0x2C,0x2D,0x2E,0x2F,0x30,0x31,0x32,0x33,0x34,
    ];
    let delegate_f4 = Address::new_delegated(10, &delegate_eth).unwrap();
    let delegate_prog = vec![0x60, 0x00, 0x60, 0x00, 0xF3];
    let _ = install_evm_contract_at(&mut h, delegate_f4, &delegate_prog).unwrap();

    let auth20: [u8; 20] = [
        0x11, 0x22, 0x33, 0x44, 0x55,
        0x66, 0x77, 0x88, 0x99, 0xAA,
        0xBB, 0xCC, 0xDD, 0xEE, 0xF0,
        0x01, 0x02, 0x03, 0x04, 0x05,
    ];
    let auth_f4 = Address::new_delegated(10, &auth20).unwrap();
    set_ethaccount_with_delegate(&mut h, auth_f4, delegate_eth).unwrap();

    h.tester.instantiate_machine(fvm_integration_tests::dummy::DummyExterns).unwrap();

    // Caller with non-zero value.
    let caller_code = make_caller_value_call(auth20, 1, 0);
    let caller = fevm::create_contract(&mut h.tester, &mut owner, &caller_code).unwrap();
    assert!(caller.msg_receipt.exit_code.is_success());
    let caller_ret = caller.msg_receipt.return_data.deserialize::<fevm::CreateReturn>().unwrap();
    let caller_addr = caller_ret.robust_address.expect("robust");
    let inv = fevm::invoke_contract(&mut h.tester, &mut owner, caller_addr, &[], fevm::DEFAULT_GAS).unwrap();

    // Expect call failure due to value transfer failure; revert data empty.
    assert!(!inv.msg_receipt.exit_code.is_success());
    let out = inv.msg_receipt.return_data.bytes().to_vec();
    assert!(out.is_empty());
}
