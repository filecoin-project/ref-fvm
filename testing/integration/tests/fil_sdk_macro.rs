use std::env;

use fvm::executor::{ApplyFailure, ApplyKind, Executor};
use fvm_integration_tests::tester::{Account, Tester};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::tuple::*;
use fvm_shared::address::Address;
use fvm_shared::bigint::BigInt;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use num_traits::Zero;

const WASM_COMPILED_PATH: &str =
    "../../target/debug/wbuild/fil_sdk_macro_actor/fil_sdk_macro_actor.compact.wasm";

/// The state object.
#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug, Default)]
pub struct State {
    pub count: u64,
}

#[test]
fn sdk_macro() {
    // Instantiate tester
    let mut tester = Tester::new(
        NetworkVersion::V15,
        StateTreeVersion::V4,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender: [Account; 1] = tester.create_accounts().unwrap();

    // Get wasm bin
    let wasm_path = env::current_dir()
        .unwrap()
        .join(WASM_COMPILED_PATH)
        .canonicalize()
        .unwrap();
    let wasm_bin = std::fs::read(wasm_path).expect("Unable to read file");

    // Set actor state
    let actor_state = State::default();
    let state_cid = tester.set_state(&actor_state).unwrap();

    // Set actor
    let actor_address = Address::new_id(10000);

    tester
        .set_actor_from_bin(&wasm_bin, state_cid, actor_address, BigInt::zero())
        .unwrap();

    // Instantiate machine
    tester.instantiate_machine().unwrap();

    // Send message
    let message = Message {
        from: sender[0].1,
        to: actor_address,
        gas_limit: 1000000000,
        method_num: 1,
        ..Message::default()
    };

    let res = tester
        .executor
        .as_mut()
        .unwrap()
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    // Test assert!
    match res.failure_info.unwrap() {
        ApplyFailure::MessageBacktrace(backtrace) => {
            assert_eq!(backtrace.frames[0].code.value(), 24);
            assert!(backtrace.frames[0].message.contains("hello world"));
        }
        _ => panic!("failure should be message backtrace"),
    }

    // Send message
    let message = Message {
        from: sender[0].1,
        to: actor_address,
        gas_limit: 1000000000,
        method_num: 2,
        sequence: 1,
        ..Message::default()
    };

    let res = tester
        .executor
        .as_mut()
        .unwrap()
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    // Test assert_eq!
    match res.failure_info.unwrap() {
        ApplyFailure::MessageBacktrace(backtrace) => {
            assert_eq!(backtrace.frames[0].code.value(), 24);
            assert!(backtrace.frames[0].message.contains("throw non equal"));
        }
        _ => panic!("failure should be message backtrace"),
    }

    // Send message
    let message = Message {
        from: sender[0].1,
        to: actor_address,
        gas_limit: 1000000000,
        method_num: 3,
        sequence: 2,
        ..Message::default()
    };

    let res = tester
        .executor
        .unwrap()
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    // Test assert_ne!
    match res.failure_info.unwrap() {
        ApplyFailure::MessageBacktrace(backtrace) => {
            assert_eq!(backtrace.frames[0].code.value(), 24);
            assert!(backtrace.frames[0].message.contains("throw equal"));
        }
        _ => panic!("failure should be message backtrace"),
    }
}
