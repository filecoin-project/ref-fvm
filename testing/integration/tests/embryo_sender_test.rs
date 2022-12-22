// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod bundles;

#[test]
fn placeholder_as_sender() {
    use bundles::*;
    use fvm::executor::{ApplyKind, Executor};
    use fvm::machine::Machine;
    use fvm_integration_tests::dummy::DummyExterns;
    use fvm_integration_tests::tester::INITIAL_ACCOUNT_BALANCE;
    use fvm_ipld_blockstore::MemoryBlockstore;
    use fvm_shared::address::Address;
    use fvm_shared::econ::TokenAmount;
    use fvm_shared::message::Message;
    use fvm_shared::state::StateTreeVersion;
    use fvm_shared::version::NetworkVersion;
    use fvm_shared::METHOD_SEND;

    // Instantiate tester
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let initial_balance = TokenAmount::from_whole(100);
    let to_send = TokenAmount::from_atto(20000);

    let [(_, receiver)] = tester.create_accounts().unwrap();

    let sender = Address::new_delegated(10, b"foobar").expect("failed to construct address");
    tester
        .create_placeholder(&sender, initial_balance.clone())
        .expect("failed to instantiate placeholder");

    // Instantiate machine
    tester.instantiate_machine(DummyExterns).unwrap();

    let executor = tester.executor.as_mut().unwrap();

    let message = Message {
        from: sender,
        to: receiver,
        gas_limit: 1000000000,
        method_num: METHOD_SEND,
        sequence: 0,
        value: to_send.clone(),
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

    let receiver_balance = tester
        .executor
        .as_ref()
        .unwrap()
        .state_tree()
        .get_actor_by_address(&receiver)
        .expect("couldn't find receiver actor")
        .expect("actor state didn't exist")
        .balance;

    assert_eq!(
        receiver_balance,
        to_send.clone() + INITIAL_ACCOUNT_BALANCE.clone()
    );

    let sender_balance = tester
        .executor
        .as_ref()
        .unwrap()
        .state_tree()
        .get_actor_by_address(&sender)
        .expect("couldn't find receiver actor")
        .expect("actor state didn't exist")
        .balance;

    assert_eq!(sender_balance, initial_balance - to_send);
}
