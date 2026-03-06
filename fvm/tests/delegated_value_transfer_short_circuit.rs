// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod common;

use cid::Cid;
use common::{install_evm_contract_at, new_harness, set_ethaccount_with_delegate};
use fvm_integration_tests::tester::{BasicAccount, ExecutionOptions};
use fvm_integration_tests::testkit::fevm;
use fvm_ipld_encoding::CborStore;
use fvm_shared::address::Address;

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
fn delegated_value_transfer_short_circuit() {
    let options = ExecutionOptions {
        debug: false,
        trace: false,
        events: true,
    };
    let mut h = new_harness(options).expect("harness");
    let mut owner: BasicAccount = h.tester.create_basic_account().unwrap();

    // Deploy a do-nothing delegate.
    let delegate_eth: [u8; 20] = [
        0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F,
        0x30, 0x31, 0x32, 0x33, 0x34,
    ];
    let delegate_f4 = Address::new_delegated(10, &delegate_eth).unwrap();
    let delegate_prog = vec![0x60, 0x00, 0x60, 0x00, 0xF3];
    let _ = install_evm_contract_at(&mut h, delegate_f4, &delegate_prog).unwrap();

    let auth20: [u8; 20] = [
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xF0,
        0x01, 0x02, 0x03, 0x04, 0x05,
    ];
    let auth_f4 = Address::new_delegated(10, &auth20).unwrap();
    let auth_id = set_ethaccount_with_delegate(&mut h, auth_f4, delegate_eth).unwrap();

    // Pre-install a caller contract with non-zero value on CALL to the authority.
    let caller_code = make_caller_value_call(auth20, 1, 0);
    let caller_eth20 = [
        0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA, 0xFB, 0xFC, 0xFD, 0xFE,
        0xEE, 0xED, 0xEC, 0xEB, 0xEA,
    ];
    let caller_f4 = Address::new_delegated(10, &caller_eth20).unwrap();
    let _ = install_evm_contract_at(&mut h, caller_f4, &caller_code).unwrap();

    // Read storage root before instantiating the machine
    #[derive(fvm_ipld_encoding::tuple::Deserialize_tuple)]
    struct EthAccountStateView {
        #[allow(dead_code)]
        delegate_to: Option<[u8; 20]>,
        #[allow(dead_code)]
        auth_nonce: u64,
        evm_storage_root: Cid,
    }
    let before_root: Cid = {
        let stree = h.tester.state_tree.as_ref().unwrap();
        let act = stree.get_actor(auth_id).unwrap().expect("actor");
        let view: Option<EthAccountStateView> = stree.store().get_cbor(&act.state).unwrap();
        view.expect("state").evm_storage_root
    };

    // Now instantiate the machine
    h.tester
        .instantiate_machine(fvm_integration_tests::dummy::DummyExterns)
        .unwrap();

    let inv = fevm::invoke_contract(&mut h.tester, &mut owner, caller_f4, &[], fevm::DEFAULT_GAS)
        .unwrap();

    // Expect call failure due to value transfer failure; revert data empty.
    assert!(!inv.msg_receipt.exit_code.is_success());
    let out = inv.msg_receipt.return_data.bytes().to_vec();
    assert!(out.is_empty());

    // Overlay should not persist on short-circuit (root unchanged)
    if let Some(stree) = h.tester.state_tree.as_ref() {
        let after_root: Cid = {
            let act = stree.get_actor(auth_id).unwrap().expect("actor");
            let view: Option<EthAccountStateView> = stree.store().get_cbor(&act.state).unwrap();
            view.expect("state").evm_storage_root
        };
        assert_eq!(
            before_root, after_root,
            "storage root should not persist on short-circuit"
        );
    }
}
// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
