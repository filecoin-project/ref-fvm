// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use bundles::*;
use fvm::executor::{ApplyKind, Executor};
use fvm::machine::Machine;
use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::tester::{BasicExecutor, INITIAL_ACCOUNT_BALANCE};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_shared::address::{Address, SECP_PUB_LEN};
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use fvm_shared::{ActorID, METHOD_SEND};

mod bundles;

fn sender_balance_and_id(
    executor: &BasicExecutor,
    sender_address: &Address,
) -> (ActorID, TokenAmount) {
    let state_tree = executor.state_tree();
    let actor = state_tree
        .get_actor_by_address(sender_address)
        .expect("failed to load sender actor")
        .expect("sender actor missing from state tree");
    let id = state_tree
        .lookup_id(sender_address)
        .expect("failed to resolve sender address")
        .expect("sender address has no ID mapping");
    (id, actor.balance)
}

#[test]
fn reservation_blocks_value_over_free_on_send() {
    let mut tester = new_tester(
        NetworkVersion::V21,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let [
        (sender_id, sender_address),
        (_receiver_id, receiver_address),
    ] = tester.create_accounts().unwrap();

    tester.instantiate_machine(DummyExterns).unwrap();
    let executor = tester.executor.as_mut().unwrap();

    // Top up the sender balance so we can use large gas limits without running out of funds when
    // building the reservation plan.
    let topup_balance = TokenAmount::from_atto(1_000_000_000u64);
    executor
        .state_tree_mut()
        .mutate_actor(sender_id, |actor| {
            actor.balance = topup_balance.clone();
            Ok(())
        })
        .expect("failed to top up sender balance");

    let (_id, balance) = sender_balance_and_id(executor, &sender_address);
    assert_eq!(balance, topup_balance);

    // Reserve exactly the cap×limit for this sender.
    let gas_fee_cap = TokenAmount::from_atto(1);
    let gas_limit = 1_000_000u64;
    let gas_cost = gas_fee_cap.clone() * gas_limit;
    let plan = vec![(sender_address, gas_cost.clone())];
    executor
        .begin_reservation_session(&plan)
        .expect("begin reservation session");

    // Free balance during the session is balance − reserved.
    let free = balance - gas_cost;
    let value = &free + TokenAmount::from_atto(1u8);

    let message = Message {
        from: sender_address,
        to: receiver_address,
        gas_limit,
        gas_fee_cap: gas_fee_cap.clone(),
        method_num: METHOD_SEND,
        sequence: 0,
        value: value.clone(),
        ..Message::default()
    };

    let res = executor
        .execute_message(message, ApplyKind::Explicit, 100)
        .expect("execution failed");

    assert_eq!(
        res.msg_receipt.exit_code,
        ExitCode::SYS_INSUFFICIENT_FUNDS,
        "send should fail when value exceeds free balance under reservations",
    );

    // Destination balance should be unchanged.
    let dest_balance = executor
        .state_tree()
        .get_actor_by_address(&receiver_address)
        .expect("failed to load receiver actor")
        .expect("receiver actor missing")
        .balance;
    assert_eq!(
        dest_balance, *INITIAL_ACCOUNT_BALANCE,
        "receiver balance should not change on failed send"
    );
}

#[test]
fn reservation_blocks_actor_creation_value_over_free() {
    let mut tester = new_tester(
        NetworkVersion::V21,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let [(sender_id, sender_address)] = tester.create_accounts().unwrap();

    tester.instantiate_machine(DummyExterns).unwrap();
    let executor = tester.executor.as_mut().unwrap();

    let topup_balance = TokenAmount::from_atto(1_000_000_000u64);
    executor
        .state_tree_mut()
        .mutate_actor(sender_id, |actor| {
            actor.balance = topup_balance.clone();
            Ok(())
        })
        .expect("failed to top up sender balance");

    let (_id, balance) = sender_balance_and_id(executor, &sender_address);
    assert_eq!(balance, topup_balance);

    let gas_fee_cap = TokenAmount::from_atto(1);
    let gas_limit = 1_000_000u64;
    let gas_cost = gas_fee_cap.clone() * gas_limit;
    let plan = vec![(sender_address, gas_cost.clone())];
    executor
        .begin_reservation_session(&plan)
        .expect("begin reservation session");

    let free = balance - gas_cost;
    let value = &free + TokenAmount::from_atto(1u8);

    // Choose a new Secp256k1 address that does not yet exist in the state tree.
    let new_addr = Address::new_secp256k1(&[1u8; SECP_PUB_LEN]).expect("invalid secp address");
    assert!(
        executor
            .state_tree()
            .get_actor_by_address(&new_addr)
            .expect("lookup failed")
            .is_none(),
        "new address unexpectedly already has an actor"
    );

    let message = Message {
        from: sender_address,
        to: new_addr,
        gas_limit,
        gas_fee_cap,
        method_num: METHOD_SEND,
        sequence: 0,
        value: value.clone(),
        ..Message::default()
    };

    let res = executor
        .execute_message(message, ApplyKind::Explicit, 100)
        .expect("execution failed");

    assert!(
        !res.msg_receipt.exit_code.is_success(),
        "actor-creation send should fail when value exceeds free balance under reservations, got {:?}",
        res.msg_receipt.exit_code,
    );

    // The auto-created account actor must not receive funds when the transfer fails.
    let created_actor = executor
        .state_tree()
        .get_actor_by_address(&new_addr)
        .expect("lookup failed");
    if let Some(actor) = created_actor {
        assert!(
            actor.balance.is_zero(),
            "newly created actor should not receive funds on failed transfer"
        );
    }
}
