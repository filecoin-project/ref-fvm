// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod bundles;
use bundles::*;
use fil_events_actor::WASM_BINARY as EVENTS_BINARY;
use fvm::executor::{ApplyKind, Executor};
use fvm::machine::Machine;
use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::tester::IntegrationExecutor;
use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
use fvm_ipld_encoding::to_vec;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use num_traits::Zero;

#[test]
fn events_test() {
    let (mut executor, sender_address, actor_address) = setup();

    // === Emits two events ===

    let message = Message {
        from: sender_address,
        to: actor_address,
        gas_limit: 1000000000,
        method_num: 2,
        sequence: 0,
        ..Message::default()
    };

    let res = executor
        .execute_message(message.clone(), ApplyKind::Explicit, 100)
        .unwrap();

    assert_eq!(ExitCode::OK, res.msg_receipt.exit_code);

    let gas_used = res.msg_receipt.gas_used;

    // Check that we got two events.
    assert_eq!(2, res.events.len());

    // Check the events AMT.
    assert!(res.msg_receipt.events_root.is_some());
    // Check that we haven't inserted the events AMT in the blockstore.
    assert!(!executor
        .blockstore()
        .has(&res.msg_receipt.events_root.unwrap())
        .unwrap());

    // === Emits an improperly formatted event ===

    let message = Message {
        method_num: 3,
        sequence: 1,
        ..message
    };

    let res = executor
        .execute_message(message.clone(), ApplyKind::Explicit, 100)
        .unwrap();

    assert_eq!(ExitCode::OK, res.msg_receipt.exit_code);
    assert!(res.msg_receipt.events_root.is_none());

    let counter: u64 = 10;

    // === Performs subcalls, each emitting 2 events and all succeeding ===
    let message = Message {
        method_num: 4,
        sequence: 2,
        params: to_vec(&counter).unwrap().into(),
        ..message
    };

    let res = executor
        .execute_message(message.clone(), ApplyKind::Explicit, 100)
        .unwrap();

    assert_eq!(ExitCode::OK, res.msg_receipt.exit_code);

    // Check that we got twenty events, 2 per actor in the chain.
    assert_eq!(20, res.events.len());

    // === Performs subcalls, each emitting 2 events and reverting ===
    let message = Message {
        method_num: 5,
        sequence: 3,
        params: to_vec(&counter).unwrap().into(),
        ..message
    };

    let res = executor
        .execute_message(message.clone(), ApplyKind::Explicit, 100)
        .unwrap();

    assert_eq!(ExitCode::OK, res.msg_receipt.exit_code);

    // Check that we got ten events events only; the events from the last five
    // actors in the call stack were discarded due to an abort.
    assert_eq!(10, res.events.len());

    // === Out of gas records no events ===
    let message = Message {
        method_num: 2,
        sequence: 4,
        gas_limit: gas_used - 1,
        ..message
    };
    let res = executor
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    assert_eq!(ExitCode::SYS_OUT_OF_GAS, res.msg_receipt.exit_code);
    assert!(res.msg_receipt.events_root.is_none());
    assert_eq!(0, res.events.len());
}

fn setup() -> (
    IntegrationExecutor<MemoryBlockstore, DummyExterns>,
    Address,
    Address,
) {
    // Instantiate tester
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let [(_sender_id, sender)] = tester.create_accounts().unwrap();

    let wasm_bin = EVENTS_BINARY.unwrap();

    // Set actor state
    let actor_state = [(); 0];
    let state_cid = tester.set_state(&actor_state).unwrap();

    // Set actor
    let actor = Address::new_id(10000);

    tester
        .set_actor_from_bin(wasm_bin, state_cid, actor, TokenAmount::zero())
        .unwrap();

    // Instantiate machine
    tester.instantiate_machine(DummyExterns).unwrap();

    let executor = tester.executor.unwrap();
    (executor, sender, actor)
}
