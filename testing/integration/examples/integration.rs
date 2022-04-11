use fvm::executor::{ApplyKind, Executor};
use fvm::state_tree::ActorState;
use fvm_integration_tests::tester::Tester;
use fvm_shared::address::Address;
use fvm_shared::bigint::BigInt;
use fvm_shared::encoding::tuple::*;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use num_traits::Zero;
use std::str::FromStr;
use wabt::wat2wasm;

const WAST: &str = r#"
;; Define anonymous module with function export named `sub`.
(module
  (func (export "sub") (param $x i32) (param $y i32) (result i32)
    ;; return x - y;
    (i32.sub
      (get_local $x) (get_local $y)
    )
  )
)
"#;

#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug)]
struct State {
    empty: bool,
}

pub fn main() {
    // Instantiate tester
    let mut tester = Tester::new(NetworkVersion::V14, StateTreeVersion::V4, 10).unwrap();

    // Get wasm bin
    let wasm_bin = wat2wasm(WAST).unwrap();

    // Set actor state
    let actor_state = State { empty: true };
    let state_cid = tester.set_state(&actor_state).unwrap();

    // Set actor
    let actor_address = Address::new_id(10000);

    tester
        .set_actor_from_bin(&wasm_bin, state_cid, actor_address.clone(), BigInt::zero())
        .unwrap();

    // Instantiate machine
    tester.instantiate_machine().unwrap();

    // Send message
    let message = Message {
        version: 0,
        from: tester.accounts[0].1.clone(),
        to: actor_address,
        sequence: 0,
        value: Default::default(),
        method_num: 1,
        params: Default::default(),
        gas_limit: 1000000000,
        gas_fee_cap: Default::default(),
        gas_premium: Default::default(),
    };

    let res = tester
        .executor
        .unwrap()
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();
    dbg!(res);
    assert!(false);
}
