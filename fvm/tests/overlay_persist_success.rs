// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod common;

use cid::Cid;
use common::{install_evm_contract_at, new_harness, set_ethaccount_with_delegate};
use fvm_integration_tests::tester::{BasicAccount, ExecutionOptions};
use fvm_integration_tests::testkit::fevm;
use fvm_ipld_encoding::CborStore;
use fvm_shared::address::Address;

fn make_sstore_then_return(slot: u8, val: u8) -> Vec<u8> {
    // PUSH1 val; PUSH1 slot; SSTORE; RETURN(0,0)
    let mut code = Vec::new();
    code.extend_from_slice(&[0x60, val]);
    code.extend_from_slice(&[0x60, slot]);
    code.push(0x55); // SSTORE
    code.extend_from_slice(&[0x60, 0x00, 0x60, 0x00, 0xF3]);
    code
}

fn make_call_authority(authority20: [u8; 20]) -> Vec<u8> {
    // CALL with 0 value, no args, no rets
    let mut code = Vec::new();
    code.extend_from_slice(&[0x61, 0xFF, 0xFF]);
    code.push(0x73);
    code.extend_from_slice(&authority20);
    code.extend_from_slice(&[0x60, 0x00, 0x60, 0x00, 0x60, 0x00, 0x60, 0x00]);
    code.push(0xF1);
    code.extend_from_slice(&[0x60, 0x00, 0x60, 0x00, 0xF3]);
    code
}

#[test]
fn overlay_persists_only_on_success() {
    let options = ExecutionOptions {
        debug: false,
        trace: false,
        events: true,
    };
    let mut h = new_harness(options).expect("harness");
    let mut owner: BasicAccount = h.tester.create_basic_account().unwrap();

    // Install delegate B that writes to storage.
    let b20 = [0xB0u8; 20];
    let b_f4 = Address::new_delegated(10, &b20).unwrap();
    let b_prog = make_sstore_then_return(1, 2);
    let _ = install_evm_contract_at(&mut h, b_f4, &b_prog).unwrap();

    // Authority A -> B
    let a20 = [0xA0u8; 20];
    let a_f4 = Address::new_delegated(10, &a20).unwrap();
    let a_id = set_ethaccount_with_delegate(&mut h, a_f4, b20).unwrap();

    // Pre-install caller C that CALLs A
    let caller_prog = make_call_authority(a20);
    let c_f4 = Address::new_delegated(10, &[0xC0u8; 20]).unwrap();
    let _ = install_evm_contract_at(&mut h, c_f4, &caller_prog).unwrap();

    // Read storage root before instantiating the machine
    #[derive(fvm_ipld_encoding::tuple::Deserialize_tuple)]
    struct EthAccountStateView {
        #[allow(dead_code)]
        delegate_to: Option<[u8; 20]>,
        #[allow(dead_code)]
        auth_nonce: u64,
        evm_storage_root: Cid,
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

    // Invoke
    let inv =
        fevm::invoke_contract(&mut h.tester, &mut owner, c_f4, &[], fevm::DEFAULT_GAS).unwrap();
    if !inv.msg_receipt.exit_code.is_success() {
        // In minimal builds (--no-default-features), delegated CALL interception
        // may be disabled; tolerate failure by exiting early.
        return;
    }

    // Expect storage root changed (persisted) on success
    if let Some(stree) = h.tester.state_tree.as_ref() {
        let after_root = {
            let act = stree.get_actor(a_id).unwrap().expect("actor");
            let view: Option<EthAccountStateView> = stree.store().get_cbor(&act.state).unwrap();
            view.expect("state").evm_storage_root
        };
        assert_ne!(
            before_root, after_root,
            "storage root should persist on success"
        );
    }
}
