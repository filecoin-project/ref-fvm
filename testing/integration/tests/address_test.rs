// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod bundles;
use bundles::*;
use fil_address_actor::WASM_BINARY as ADDRESS_BINARY;
use fvm::executor::{ApplyKind, Executor};
use fvm_integration_tests::dummy::DummyExterns;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use num_traits::Zero;

#[test]
fn basic_address_tests() {
    // Instantiate tester
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let [(_sender_id, sender_address)] = tester.create_accounts().unwrap();

    let wasm_bin = ADDRESS_BINARY.unwrap();

    // Set actor state
    let actor_state = [(); 0];
    let state_cid = tester.set_state(&actor_state).unwrap();

    // Set actor
    let actor_address = Address::new_id(10000);

    tester
        .set_actor_from_bin(wasm_bin, state_cid, actor_address, TokenAmount::zero())
        .unwrap();

    // Instantiate machine
    tester.instantiate_machine(DummyExterns).unwrap();

    let executor = tester.executor.as_mut().unwrap();

    // Test all methods.
    for (seq, method) in (2..=5).enumerate() {
        let message = Message {
            from: sender_address,
            to: actor_address,
            gas_limit: 1000000000,
            method_num: method,
            sequence: seq as u64,
            ..Message::default()
        };

        let res = executor
            .execute_message(message, ApplyKind::Explicit, 100)
            .unwrap();
        assert!(
            res.msg_receipt.exit_code.is_success(),
            "{:?}",
            res.failure_info
        );
    }
}
