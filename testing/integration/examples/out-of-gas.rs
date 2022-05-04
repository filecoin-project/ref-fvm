use fvm::executor::{ApplyKind, Executor};
use fvm_integration_tests::tester::{Account, Tester};
use fvm_ipld_encoding::tuple::*;
use fvm_shared::address::Address;
use fvm_shared::bigint::BigInt;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use num_traits::Zero;
use wabt::wat2wasm;

const WAT: &str = r#"
;; Mock invoke function
(module
  (func (export "invoke") (param $x i32) (result i32)
    (loop
        (br 0)
    )
    (i32.const 1)
  )
)
"#;

#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug)]
struct State {
    empty: bool,
}

pub fn main() {
    println!("start");

    // Instantiate tester
    let mut tester = Tester::new(NetworkVersion::V16, StateTreeVersion::V4).unwrap();

    let sender: [Account; 1] = tester.create_account().unwrap();

    // Get wasm bin
    let wasm_bin = wat2wasm(WAT).unwrap();

    // Set actor state
    let actor_state = State { empty: true };
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
        gas_limit: 10_000_000,
        method_num: 1,
        ..Message::default()
    };

    println!("exec");

    let res = tester
        .executor
        .unwrap()
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    println!("r: {:?}", res);
}
