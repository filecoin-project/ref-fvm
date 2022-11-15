mod bundles;
use bundles::*;
use fil_events_actor::WASM_BINARY as EVENTS_BINARY;
use fvm::executor::{ApplyKind, Executor};
use fvm::machine::Machine;
use fvm_integration_tests::dummy::DummyExterns;
use fvm_ipld_amt::Amt;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::to_vec;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::event::StampedEvent;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use num_traits::Zero;

#[test]
fn events_test() {
    // Instantiate tester
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let [(_sender_id, sender_address)] = tester.create_accounts().unwrap();

    let wasm_bin = EVENTS_BINARY.unwrap();

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

    // Check that we got two events.
    assert_eq!(2, res.events.len());

    // Check the events AMT.
    assert!(res.msg_receipt.events_root.is_some());
    let events_amt: Amt<StampedEvent, _> =
        Amt::load(&res.msg_receipt.events_root.unwrap(), executor.blockstore()).unwrap();
    assert_eq!(2, events_amt.count());

    // Check that events in the AMT match events returned in ApplyRet.
    for (i, evt) in res.events.iter().enumerate() {
        assert_eq!(Some(evt), events_amt.get(i as u64).unwrap());
    }

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
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    assert_eq!(ExitCode::OK, res.msg_receipt.exit_code);

    // Check that we got ten events events only; the events from the last five
    // actors in the call stack were discarded due to an abort.
    assert_eq!(10, res.events.len());
}
