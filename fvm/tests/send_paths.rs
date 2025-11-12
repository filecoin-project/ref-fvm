// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm::executor::{ApplyKind, Executor};
use fvm_integration_tests::tester::ExecutionOptions;
use fvm_shared::METHOD_SEND;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::message::Message;

mod common;
use common::new_harness;

#[test]
fn send_creates_placeholder_and_transfers() {
    let options = ExecutionOptions { debug: false, trace: false, events: true };
    let mut h = new_harness(options).expect("harness");
    let (_sender_id, sender_addr) = h.tester.create_account().unwrap();

    // Pre-choose a delegated recipient; first send (0 value) creates placeholder, second transfers.
    let to = Address::new_delegated(10, &[0xAAu8; 20]).unwrap();

    h.tester.instantiate_machine(fvm_integration_tests::dummy::DummyExterns).unwrap();
    let exec = h.tester.executor.as_mut().unwrap();

    // Create placeholder (0 value)
    let msg0 = Message { from: sender_addr, to, method_num: METHOD_SEND, value: TokenAmount::from_atto(0u8), gas_limit: 10_000_000, ..Message::default() };
    let ret0 = exec.execute_message(msg0, ApplyKind::Explicit, 100).unwrap();
    assert!(ret0.msg_receipt.exit_code.is_success());

    // Transfer non-zero value to existing placeholder actor
    let msg1 = Message { from: sender_addr, to, method_num: METHOD_SEND, value: TokenAmount::from_atto(1u8), gas_limit: 10_000_000, sequence: 1, ..Message::default() };
    let ret1 = exec.execute_message(msg1, ApplyKind::Explicit, 100).unwrap();
    assert!(ret1.msg_receipt.exit_code.is_success());
}

#[test]
fn send_creates_bls_account_and_transfers() {
    let options = ExecutionOptions { debug: false, trace: false, events: true };
    let mut h = new_harness(options).expect("harness");
    let (_sender_id, sender_addr) = h.tester.create_account().unwrap();

    // Create a synthetic BLS key address (48 bytes payload)
    let to = Address::new_bls(&[0x11u8; 48]).unwrap();

    h.tester.instantiate_machine(fvm_integration_tests::dummy::DummyExterns).unwrap();
    let exec = h.tester.executor.as_mut().unwrap();

    // Auto-create account actor (0 value)
    let msg0 = Message { from: sender_addr, to, method_num: METHOD_SEND, value: TokenAmount::from_atto(0u8), gas_limit: 10_000_000, ..Message::default() };
    let ret0 = exec.execute_message(msg0, ApplyKind::Explicit, 100).unwrap();
    assert!(ret0.msg_receipt.exit_code.is_success());

    // Transfer non-zero value to the newly created account actor
    let msg1 = Message { from: sender_addr, to, method_num: METHOD_SEND, value: TokenAmount::from_atto(2u8), gas_limit: 10_000_000, sequence: 1, ..Message::default() };
    let ret1 = exec.execute_message(msg1, ApplyKind::Explicit, 100).unwrap();
    assert!(ret1.msg_receipt.exit_code.is_success());
}
