// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod bundles;
use bundles::*;
use fvm::executor::{ApplyKind, Executor};
use fvm::machine::Machine;
use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::tester::IntegrationExecutor;
use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
use fvm_ipld_encoding::{to_vec, IPLD_RAW};
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::event::{Entry, Flags, StampedEvent};
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use fvm_test_actors::wasm_bin::EVENTS_ACTOR_BINARY;
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

    assert!(
        res.msg_receipt.exit_code.is_success(),
        "{:?}",
        res.failure_info
    );

    let gas_used = res.msg_receipt.gas_used;

    // Assert that we got the correct events.
    let actor_id = actor_address.id().unwrap();
    assert_eq!(
        &res.events,
        &[
            StampedEvent {
                emitter: actor_id,
                event: vec![Entry {
                    flags: Flags::all(),
                    key: "foo".to_owned(),
                    codec: IPLD_RAW,
                    value: "abc".into(),
                },]
                .into(),
            },
            StampedEvent {
                emitter: actor_id,
                event: vec![
                    Entry {
                        flags: Flags::all(),
                        key: "bar".to_owned(),
                        codec: IPLD_RAW,
                        value: "def".into(),
                    },
                    Entry {
                        flags: Flags::FLAG_INDEXED_KEY | Flags::FLAG_INDEXED_VALUE,
                        key: "👱".to_string(),
                        codec: IPLD_RAW,
                        value: "123456789 abcdefg 123456789".into(),
                    },
                ]
                .into(),
            },
        ]
    );

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
        NetworkVersion::V21,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let [(_sender_id, sender)] = tester.create_accounts().unwrap();

    let wasm_bin = EVENTS_ACTOR_BINARY;

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
