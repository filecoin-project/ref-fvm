// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod bundles;
use bundles::*;
use fvm::executor::{ApplyKind, Executor};
use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::tester::Account;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use fvm_test_actors::wasm_bin::UPGRADE_ACTOR_BINARY;
use num_traits::Zero;

#[test]
fn upgrade_actor_test() {
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender: [Account; 3] = tester.create_accounts().unwrap();
    let receiver = Address::new_id(10000);
    let state_cid = tester.set_state(&[(); 0]).unwrap();

    let wasm_bin = UPGRADE_ACTOR_BINARY;
    tester
        .set_actor_from_bin(wasm_bin, state_cid, receiver, TokenAmount::zero())
        .unwrap();
    tester.instantiate_machine(DummyExterns).unwrap();

    let executor = tester.executor.as_mut().unwrap();

    {
        // test a successful call to `upgrade` endpoint
        let message = Message {
            from: sender[0].1,
            to: receiver,
            gas_limit: 1000000000,
            method_num: 1,
            sequence: 0_u64,
            value: TokenAmount::from_atto(100),
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
        let val: i64 = res.msg_receipt.return_data.deserialize().unwrap();
        assert_eq!(val, 666);
    }

    {
        let message = Message {
            from: sender[1].1,
            to: receiver,
            gas_limit: 1000000000,
            method_num: 2,
            sequence: 0_u64,
            value: TokenAmount::from_atto(100),
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

    {
        let message = Message {
            from: sender[2].1,
            to: receiver,
            gas_limit: 1000000000,
            method_num: 3,
            sequence: 0_u64,
            value: TokenAmount::from_atto(100),
            ..Message::default()
        };

        let res = executor
            .execute_message(message, ApplyKind::Explicit, 100)
            .unwrap();

        let val: i64 = res.msg_receipt.return_data.deserialize().unwrap();
        assert_eq!(val, 444);

        assert!(
            res.msg_receipt.exit_code.is_success(),
            "{:?}",
            res.failure_info
        );
    }
}
