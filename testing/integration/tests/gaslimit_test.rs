// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use bundles::*;
use fil_gaslimit_actor::WASM_BINARY as BINARY;
use fvm::executor::{ApplyKind, Executor};
use fvm::machine::Machine;
use fvm_integration_tests::dummy::DummyExterns;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::to_vec;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use num_traits::Zero;
use serde_tuple::*;

mod bundles;

#[test]
fn gaslimit_test() {
    // Instantiate tester
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let [(_sender_id, sender_address), (_dest_id, dest_address)] =
        tester.create_accounts().unwrap();

    // Set actor
    let actor_address = {
        let addr = Address::new_id(10000);
        let actor_state = [(); 0];
        let state_cid = tester.set_state(&actor_state).unwrap();
        let wasm_bin = BINARY.unwrap();
        tester
            .set_actor_from_bin(wasm_bin, state_cid, addr, TokenAmount::zero())
            .unwrap();
        addr
    };

    // Instantiate machine
    tester.instantiate_machine(DummyExterns).unwrap();

    let executor = tester.executor.as_mut().unwrap();

    #[derive(Serialize_tuple, Deserialize_tuple, PartialEq, Eq, Clone, Debug)]
    struct Params {
        dest: Address,
        inner_gas_limit: u64,
        exhaust: bool,
        expect_err: bool,
    }

    //
    // SCENARIO A: with a child gas limit which is exceeded; inner send reverted, no event published.
    //
    let params = Params {
        dest: dest_address,
        inner_gas_limit: 10000000,
        exhaust: true,
        expect_err: true,
    };

    let message = Message {
        from: sender_address,
        to: actor_address,
        gas_limit: 1000000000,
        method_num: 2,
        sequence: 0,
        value: TokenAmount::from_atto(100),
        params: to_vec(&params).unwrap().into(),
        ..Message::default()
    };
    let res = executor
        .execute_message(message.clone(), ApplyKind::Explicit, 100)
        .unwrap();
    assert_eq!(
        ExitCode::OK,
        res.msg_receipt.exit_code,
        "{:?}",
        res.failure_info
    );
    assert_eq!(0, res.events.len());
    assert!(res.msg_receipt.events_root.is_none());
    assert_eq!(
        TokenAmount::from_atto(10050), // got 50 from actor, starting with 10000 initial balance
        executor
            .state_tree()
            .get_actor_by_address(&dest_address)
            .unwrap()
            .unwrap()
            .balance
    );

    //
    // SCENARIO B: with a child gas limit which is not exceeded; inner send preserved, event published.
    //
    let params = Params {
        dest: dest_address,
        inner_gas_limit: 10000000,
        exhaust: false,
        expect_err: false,
    };
    let message = Message {
        sequence: 1,
        params: to_vec(&params).unwrap().into(),
        ..message
    };
    let res = executor
        .execute_message(message.clone(), ApplyKind::Explicit, 100)
        .unwrap();
    assert_eq!(ExitCode::OK, res.msg_receipt.exit_code);
    assert_eq!(1, res.events.len());
    assert!(res.msg_receipt.events_root.is_some());
    assert_eq!(
        TokenAmount::from_atto(10110), // got another 60 from actor
        executor
            .state_tree()
            .get_actor_by_address(&dest_address)
            .unwrap()
            .unwrap()
            .balance
    );

    //
    // SCENARIO C: with no child gas limit; inner send preserved, event published.
    //
    let params = Params {
        dest: dest_address,
        inner_gas_limit: 0,
        exhaust: false,
        expect_err: false,
    };
    let message = Message {
        sequence: 2,
        params: to_vec(&params).unwrap().into(),
        ..message
    };
    let res = executor
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();
    assert_eq!(ExitCode::OK, res.msg_receipt.exit_code);
    assert_eq!(1, res.events.len());
    assert!(res.msg_receipt.events_root.is_some());
    assert_eq!(
        TokenAmount::from_atto(10170), // got another 60 from actor
        executor
            .state_tree()
            .get_actor_by_address(&dest_address)
            .unwrap()
            .unwrap()
            .balance
    );
}
