use fvm::executor::{ApplyKind, Executor};
use fvm_integration_tests::tester::Tester;
use fvm_shared::address::Address;
use fvm_shared::bigint::BigInt;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use num_traits::Zero;
use syscall::State;

pub mod wasm {
    include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_wasm_binaries() {
            assert!(!WASM_BINARY.unwrap().is_empty());
            assert!(!WASM_BINARY_BLOATY.unwrap().is_empty());
        }
    }
}

#[test]
fn it_works() {
    // Instantiate tester
    let (mut tester, mut state_tree) =
        Tester::new(NetworkVersion::V15, StateTreeVersion::V4, 10).unwrap();

    // Get wasm bin
    let wasm_bin = crate::wasm::WASM_BINARY.unwrap();

    // Set actor state
    let actor_state = State::default();
    let state_cid = tester.set_state(&mut state_tree, &actor_state).unwrap();

    // Set actor
    let actor_address = Address::new_id(10000);

    tester
        .set_actor_from_bin(
            &mut state_tree,
            &wasm_bin,
            state_cid,
            actor_address,
            BigInt::zero(),
        )
        .unwrap();

    // Instantiate machine
    tester.instantiate_machine(state_tree).unwrap();

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
    assert!(false)
}
