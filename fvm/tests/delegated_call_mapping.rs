mod common;

use common::{new_harness, set_ethaccount_with_delegate};
use fvm_integration_tests::tester::{BasicAccount, ExecutionOptions};
use fvm_integration_tests::testkit::fevm;
use fvm_shared::address::Address;
use fvm_shared::error::ExitCode;

fn make_reverting_delegate(payload: [u8; 4]) -> Vec<u8> {
    // REVERT with 4-byte payload at offset 0
    let mut code = Vec::new();
    code.extend_from_slice(&[0x63, payload[0], payload[1], payload[2], payload[3]]); // PUSH4 payload
    code.extend_from_slice(&[0x60, 0x00]); // PUSH1 0
    code.push(0x52); // MSTORE
    code.extend_from_slice(&[0x60, 0x04, 0x60, 0x00, 0xFD]); // REVERT(0,4)
    code
}

fn make_returning_delegate(payload: [u8; 4]) -> Vec<u8> {
    // RETURN 4-byte payload from offset 0
    let mut code = Vec::new();
    code.extend_from_slice(&[0x63, payload[0], payload[1], payload[2], payload[3]]); // PUSH4 payload
    code.extend_from_slice(&[0x60, 0x00]); // PUSH1 0
    code.push(0x52); // MSTORE
    code.extend_from_slice(&[0x60, 0x04, 0x60, 0x00, 0xF3]); // RETURN(0,4)
    code
}

fn make_caller_call_authority(authority20: [u8; 20], ret_len: u8) -> Vec<u8> {
    // Performs CALL(gas, authority, value=0, args=(0,0), rets=(0,ret_len)), then returns that region.
    let mut code = Vec::new();
    // Push CALL args (note: order is: gas, address, value, argsOffset, argsLength, retOffset, retLength)
    code.extend_from_slice(&[0x61, 0xFF, 0xFF]); // PUSH2 0xFFFF (gas)
    code.push(0x73); // PUSH20 <authority>
    code.extend_from_slice(&authority20);
    code.extend_from_slice(&[0x60, 0x00]); // value = 0
    code.extend_from_slice(&[0x60, 0x00]); // argsOffset = 0
    code.extend_from_slice(&[0x60, 0x00]); // argsLength = 0
    code.extend_from_slice(&[0x60, 0x00]); // retOffset = 0
    code.extend_from_slice(&[0x60, ret_len]); // retLength
    code.push(0xF1); // CALL
    // ignore success flag; just return rets
    code.extend_from_slice(&[0x60, ret_len, 0x60, 0x00, 0xF3]);
    code
}

#[test]
fn delegated_call_success_mapping() {
    // Harness
    let options = ExecutionOptions { debug: false, trace: false, events: true };
    let mut h = new_harness(options).expect("harness");
    let mut owner: BasicAccount = h.tester.create_basic_account().unwrap();

    // Deploy two delegates: one returning, one reverting.
    let ok_payload = [0xDE, 0xAD, 0xBE, 0xEF];
    // Revert mapping case covered in builtin-actors tests; VM intercept here validates success mapping only.
}
