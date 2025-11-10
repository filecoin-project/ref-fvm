mod common;

use common::{new_harness, set_ethaccount_with_delegate, install_evm_contract_at};
use fvm_integration_tests::tester::{BasicAccount, ExecutionOptions};
use fvm_integration_tests::testkit::fevm;
use fvm_shared::address::Address;

fn returning_const(val: [u8; 4]) -> Vec<u8> {
    let mut code = Vec::new();
    code.extend_from_slice(&[0x63, val[0], val[1], val[2], val[3]]);
    code.extend_from_slice(&[0x60, 0x00]);
    code.push(0x52);
    code.extend_from_slice(&[0x60, 0x04, 0x60, 0x00, 0xF3]);
    code
}

fn caller_call_authority(auth20: [u8; 20]) -> Vec<u8> {
    // CALL with ret_len=4 and return rets
    let mut code = Vec::new();
    code.extend_from_slice(&[0x61, 0xFF, 0xFF]);
    code.push(0x73);
    code.extend_from_slice(&auth20);
    code.extend_from_slice(&[0x60, 0x00]); // value=0
    code.extend_from_slice(&[0x60, 0x00]); // argsOff
    code.extend_from_slice(&[0x60, 0x00]); // argsLen
    code.extend_from_slice(&[0x60, 0x00]); // retOff
    code.extend_from_slice(&[0x60, 0x04]); // retLen
    code.push(0xF1);
    code.extend_from_slice(&[0x60, 0x04, 0x60, 0x00, 0xF3]);
    code
}

#[test]
fn delegated_call_depth_limit_enforced() {
    let options = ExecutionOptions { debug: false, trace: false, events: true };
    let mut h = new_harness(options).expect("harness");
    let mut owner: BasicAccount = h.tester.create_basic_account().unwrap();

    // Deploy B and C returning different constants.
    let b_val = [0xBA, 0xDD, 0xF0, 0x0D];
    let c_val = [0xCA, 0xFE, 0xBA, 0xBE];
    let b_eth20 = [0xB0,0xB1,0xB2,0xB3,0xB4,0xB5,0xB6,0xB7,0xB8,0xB9,0xBA,0xBB,0xBC,0xBD,0xBE,0xBF,0xC0,0xC1,0xC2,0xC3];
    let c_eth20 = [0xC0,0xC1,0xC2,0xC3,0xC4,0xC5,0xC6,0xC7,0xC8,0xC9,0xCA,0xCB,0xCC,0xCD,0xCE,0xCF,0xD0,0xD1,0xD2,0xD3];
    let b_f4 = Address::new_delegated(10, &b_eth20).unwrap();
    let c_f4 = Address::new_delegated(10, &c_eth20).unwrap();
    let b_rt = returning_const(b_val);
    let c_rt = returning_const(c_val);
    let _ = install_evm_contract_at(&mut h, b_f4.clone(), &b_rt).unwrap();
    let _ = install_evm_contract_at(&mut h, c_f4.clone(), &c_rt).unwrap();

    // Set A->B, B->C via EthAccount state.
    let a20: [u8; 20] = [
        0x10, 0x20, 0x30, 0x40, 0x50,
        0x60, 0x70, 0x80, 0x90, 0xA0,
        0xA1, 0xB2, 0xC3, 0xD4, 0xE5,
        0xF6, 0x01, 0x23, 0x45, 0x67,
    ];
    let b20 = b_eth20;
    let c20 = c_eth20;
    let a_f4 = Address::new_delegated(10, &a20).unwrap();
    let b_f4 = Address::new_delegated(10, &b20).unwrap();
    set_ethaccount_with_delegate(&mut h, a_f4, b20).unwrap();
    set_ethaccount_with_delegate(&mut h, b_f4, c20).unwrap();

    h.tester.instantiate_machine(fvm_integration_tests::dummy::DummyExterns).unwrap();

    // Caller -> CALL A expects to execute B only (depth=1), returning b_val.
    let caller_prog = caller_call_authority(a20);
    let caller = fevm::create_contract(&mut h.tester, &mut owner, &caller_prog).unwrap();
    let caller_ret = caller.msg_receipt.return_data.deserialize::<fevm::CreateReturn>().unwrap();
    let caller_addr = caller_ret.robust_address.expect("robust");
    let inv = fevm::invoke_contract(&mut h.tester, &mut owner, caller_addr, &[], fevm::DEFAULT_GAS).unwrap();
    assert!(inv.msg_receipt.exit_code.is_success());
    let out = inv.msg_receipt.return_data.bytes().to_vec();
    assert_eq!(out, b_val, "should stop at first delegation depth");
}
