mod bundles;

use bundles::*;
use fvm::executor::{ApplyKind, Executor};
use fvm::machine::Machine;
use fvm_integration_tests::dummy::DummyExterns;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use fvm_shared::METHOD_SEND;

#[cfg(feature = "f4-as-account")]
#[test]
fn embryo_as_sender() {
    // Instantiate tester
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V4,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender = Address::new_delegated(10, b"foobar").expect("failed to construct address");
    tester
        .create_embryo(&sender, TokenAmount::from_whole(100))
        .expect("failed to instantiate embryo");

    let [(_, receiver)] = tester.create_accounts().unwrap();

    // Instantiate machine
    tester.instantiate_machine(DummyExterns).unwrap();

    let executor = tester.executor.as_mut().unwrap();

    let message = Message {
        from: sender.clone(),
        to: receiver.clone(),
        gas_limit: 1000000000,
        method_num: METHOD_SEND,
        sequence: 0,
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

    let balance = tester
        .executor
        .unwrap()
        .state_tree()
        .get_actor(&receiver)
        .expect("couldn't find receiver actor")
        .expect("actor state didn't exist")
        .balance;

    println!("{}", balance)
}
