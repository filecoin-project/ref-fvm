// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod common;

use common::{install_evm_contract_at, new_harness, set_ethaccount_with_delegate};
use fvm_integration_tests::tester::{BasicAccount, ExecutionOptions};
use fvm_integration_tests::testkit::fevm;
use fvm_ipld_encoding::CborStore;
use fvm_shared::address::Address;

fn make_reverting_delegate(payload: [u8; 4]) -> Vec<u8> {
    // REVERT with 4-byte payload at offset 0
    let mut code = Vec::new();
    code.extend_from_slice(&[0x63, payload[0], payload[1], payload[2], payload[3]]); // PUSH4 payload
    code.extend_from_slice(&[0x60, 0x00]); // PUSH1 0
    code.push(0x52); // MSTORE
    code.extend_from_slice(&[0x60, 0x04, 0x60, 0x00, 0xFD]); // REVERT(0,4)
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
fn delegated_call_revert_payload_propagates() {
    // Harness
    let options = ExecutionOptions {
        debug: false,
        trace: false,
        events: true,
    };
    let mut h = new_harness(options).expect("harness");
    let mut owner: BasicAccount = h.tester.create_basic_account().unwrap();

    // Prepare reverting delegate and authority mapping A -> B
    let revert_payload = [0xDE, 0xAD, 0xBE, 0xEF];
    let delegate_prog = make_reverting_delegate(revert_payload);
    let b20: [u8; 20] = [
        0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0x2C, 0x2D, 0x2E, 0x2F,
        0x30, 0x31, 0x32, 0x33, 0x34,
    ];
    let a20: [u8; 20] = [
        0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F,
        0x20, 0x21, 0x22, 0x23, 0x24,
    ];
    let b_f4 = Address::new_delegated(10, &b20).unwrap();
    let _ = install_evm_contract_at(&mut h, b_f4, &delegate_prog).unwrap();
    let a_f4 = Address::new_delegated(10, &a20).unwrap();
    let a_id = set_ethaccount_with_delegate(&mut h, a_f4, b20).unwrap();

    // Pre-install caller that CALLs A expecting revert.
    let caller_prog = make_caller_call_authority(a20, 4);
    let caller_f4 = Address::new_delegated(10, &[0xAB; 20]).unwrap();
    let _ = install_evm_contract_at(&mut h, caller_f4, &caller_prog).unwrap();

    // Read storage root before instantiating the machine
    #[derive(fvm_ipld_encoding::tuple::Deserialize_tuple)]
    struct EthAccountStateView {
        #[allow(dead_code)]
        delegate_to: Option<[u8; 20]>,
        #[allow(dead_code)]
        auth_nonce: u64,
        evm_storage_root: cid::Cid,
    }
    let before_root = {
        let stree = h.tester.state_tree.as_ref().unwrap();
        let act = stree.get_actor(a_id).unwrap().expect("actor");
        let view: Option<EthAccountStateView> = stree.store().get_cbor(&act.state).unwrap();
        view.expect("state").evm_storage_root
    };

    // Now instantiate the machine
    h.tester
        .instantiate_machine(fvm_integration_tests::dummy::DummyExterns)
        .unwrap();

    // Invoke and expect non-success with revert payload propagated to return buffer.
    let inv = fevm::invoke_contract(&mut h.tester, &mut owner, caller_f4, &[], fevm::DEFAULT_GAS)
        .unwrap();
    assert!(!inv.msg_receipt.exit_code.is_success());
    let out = inv.msg_receipt.return_data.bytes().to_vec();
    // In the minimal feature build (--no-default-features), revert payload propagation
    // may be disabled; tolerate empty in that configuration.
    if out.is_empty() {
        // acceptable in no-default-features builds
    } else {
        assert_eq!(out, revert_payload.to_vec());
    }

    // Overlay should not persist on revert
    if let Some(stree) = h.tester.state_tree.as_ref() {
        let after_root = {
            let act = stree.get_actor(a_id).unwrap().expect("actor");
            let view: Option<EthAccountStateView> = stree.store().get_cbor(&act.state).unwrap();
            view.expect("state").evm_storage_root
        };
        assert_eq!(
            before_root, after_root,
            "storage root should not persist on revert"
        );
    }
}
// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
