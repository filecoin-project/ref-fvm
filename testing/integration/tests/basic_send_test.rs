// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#![cfg(test)]

mod bundles;
use bundles::*;
use fvm::executor::{ApplyKind, Executor};
use fvm::gas::GasCharge;
use fvm::machine::Machine;
use fvm_integration_tests::dummy::DummyExterns;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use fvm_shared::METHOD_SEND;

#[test]
fn basic_send() {
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let (_, sender) = tester.create_account().unwrap();

    // Send to an f4 to create a placeholder. Otherwise, we end up invoking a constructor.
    let receiver = Address::new_delegated(10, b"foobar").expect("failed to construct f4 address");

    tester.instantiate_machine(DummyExterns).unwrap();
    let executor = tester.executor.as_mut().unwrap();

    struct Case {
        to: Address,
        value: u64,
        trace: Vec<GasCharge>,
    }

    let cases = {
        let pl = executor.context().price_list;
        [
            // Create the actor.
            Case {
                to: receiver,
                value: 0,
                trace: vec![
                    // No explicit charges for updating/looking this up.
                    pl.on_chain_message(100),
                    // Create the actor. We do charge for the update/lookup because it didn't exist.
                    pl.on_create_actor(true),
                    pl.on_actor_lookup(),
                    pl.on_actor_update(),
                ],
            },
            // Poke it. Don't charge for an update because we don't transfer value.
            Case {
                to: receiver,
                value: 0,
                trace: vec![
                    // No explicit charges for updating/looking this up.
                    pl.on_chain_message(100),
                    // No charges because we're not transferring value or executing code.
                ],
            },
            // Transfer value, update the target actor.
            Case {
                to: receiver,
                value: 1,
                trace: vec![
                    // No explicit charges for updating/looking this up.
                    pl.on_chain_message(100),
                    // Transfer
                    pl.on_value_transfer(),
                    // Charge to update the target actor due to the value transfer.
                    pl.on_actor_update(),
                ],
            },
            // Transfer value to a system actor. We don't expect a state-update charge in this case.
            Case {
                to: Address::new_id(10),
                value: 1,
                trace: vec![
                    // No explicit charges for updating/looking this up.
                    pl.on_chain_message(100),
                    // Transfer
                    pl.on_value_transfer(),
                ],
            },
        ]
    };

    for (i, case) in cases.into_iter().enumerate() {
        let message = Message {
            from: sender,
            to: case.to,
            gas_limit: 1000000000,
            method_num: METHOD_SEND,
            sequence: i as u64,
            value: TokenAmount::from_atto(case.value),
            ..Message::default()
        };

        let res = executor
            .execute_message(message, ApplyKind::Explicit, 100)
            .unwrap();
        assert!(res.msg_receipt.exit_code.is_success());

        let charges: Vec<_> = res
            .exec_trace
            .into_iter()
            .filter_map(|x| match x {
                fvm::trace::ExecutionEvent::GasCharge(charge) => Some(charge),
                _ => None,
            })
            .collect();

        assert_eq!(charges, case.trace);
    }
}
