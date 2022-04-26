use std::env;

use fvm::executor::{ApplyKind, Executor};
use fvm_integration_tests::tester::{Account, Tester};
use fvm_ipld_encoding::tuple::*;
use fvm_shared::address::Address;
use fvm_shared::bigint::BigInt;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use num_traits::Zero;

/// The state object.
#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug, Default)]
pub struct State {
    pub count: u64,
}

const WASM_COMPILED_PATH: &str =
    "../../target/debug/wbuild/fil_hello_world_actor/fil_hello_world_actor.compact.wasm";

#[test]
fn hello_world() {
    // Instantiate tester
    let mut tester = Tester::new(NetworkVersion::V15, StateTreeVersion::V4).unwrap();

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
        .unwrap()
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    assert_eq!(res.msg_receipt.exit_code.value(), 16)
}
