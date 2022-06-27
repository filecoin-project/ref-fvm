use fvm::call_manager::backtrace::Cause;
use fvm::executor::{ApplyFailure, ApplyKind, Executor};
use fvm_integration_tests::tester::{Account, Tester};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::RawBytes;
use fvm_shared::address::Address;
use fvm_shared::bigint::BigInt;
use fvm_shared::error::ErrorNumber;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use num_traits::Zero;
use wabt::wat2wasm;

const WAT_UNKNOWN_SYSCALL: &str = r#"
    (module
        (type $t0 (func))
        (type $t1 (func (param i32) (result i32)))
        ;; Non existing syscall
        (import "vm" "do_not_exist" (func $fvm_sdk::sys::vm::do_not_exist::syscall (type $t0)))
        (func $invoke (export "invoke") (type $t1) (param $p0 i32) (result i32)
            (call $fvm_sdk::sys::vm::do_not_exist::syscall)
            (unreachable))
        (memory $memory (export "memory") 16)
        (global $__data_end (export "__data_end") i32 (i32.const 1048576))
        (global $__heap_base (export "__heap_base") i32 (i32.const 1048576)))
    "#;

const WASM_COMPILED_PATH: &str =
    "../../target/debug/wbuild/fil_malformed_syscall_actor/fil_malformed_syscall_actor.compact.wasm";

#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug, Default)]
pub struct State {
    pub count: i64,
}

// Utility function to instantiation integration tester
fn instantiate_tester(wasm_bin: &[u8]) -> (Account, Tester<MemoryBlockstore>, Address) {
    // Instantiate tester
    let mut tester = Tester::new(
        NetworkVersion::V15,
        StateTreeVersion::V4,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender: [Account; 1] = tester.create_accounts().unwrap();

    // Set actor state
    let actor_state = State::default();
    let state_cid = tester.set_state(&actor_state).unwrap();

    // Set actor
    let actor_address = Address::new_id(10000);

    tester
        .set_actor_from_bin(wasm_bin, state_cid, actor_address, BigInt::zero())
        .unwrap();

    (sender[0], tester, actor_address)
}

#[test]
fn non_existing_syscall() {
    // Get wasm bin
    let wasm_bin = wat2wasm(WAT_UNKNOWN_SYSCALL).unwrap();

    // Instantiate tester
    let (sender, mut tester, actor_address) = instantiate_tester(&wasm_bin);

    // Instantiate machine
    tester.instantiate_machine().unwrap();

    // Params setup
    let params = RawBytes::new(Vec::<u8>::new());

    // Send message to set
    let message = Message {
        from: sender.1,
        to: actor_address,
        gas_limit: 1000000000,
        method_num: 1,
        params,
        ..Message::default()
    };

    // Set inner state value
    let res = tester
        .executor
        .as_mut()
        .unwrap()
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    // Should be an internal VM assertion failed exit code
    assert_eq!(
        res.msg_receipt.exit_code.value(),
        10,
        "exit code should be internal VM assertion failed"
    );

    // Should be unknown import
    match res.failure_info.as_ref().unwrap() {
        ApplyFailure::MessageBacktrace(backtrace) => {
            assert!(
                backtrace
                    .cause
                    .as_ref()
                    .unwrap()
                    .to_string()
                    .contains("unknown import"),
                "error cause should be unknown import"
            );
        }
        _ => panic!("transaction result should have a backtrace"),
    }
}

#[test]
fn malformed_syscall_parameter() {
    // Get wasm bin
    let wasm_path = std::env::current_dir()
        .unwrap()
        .join(WASM_COMPILED_PATH)
        .canonicalize()
        .unwrap();

    let wasm_bin = std::fs::read(wasm_path).expect("Unable to read file");

    // Instantiate tester
    let (sender, mut tester, actor_address) = instantiate_tester(&wasm_bin);

    // Instantiate machine
    tester.instantiate_machine().unwrap();

    // Params setup
    let params = RawBytes::new(Vec::<u8>::new());

    // Send message to set
    let message = Message {
        from: sender.1,
        to: actor_address,
        gas_limit: 1000000000,
        method_num: 1,
        params,
        ..Message::default()
    };

    // Set inner state value
    let res = tester
        .executor
        .as_mut()
        .unwrap()
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    // Actor should panic
    assert_eq!(res.msg_receipt.exit_code.value(), 4);

    // Should be unknown import
    match res.failure_info.as_ref().unwrap() {
        ApplyFailure::MessageBacktrace(backtrace) => match backtrace.cause.as_ref().unwrap() {
            Cause::Syscall { error, message, .. } => {
                assert!(message.contains("invalid proof type"));

                match error {
                    ErrorNumber::IllegalArgument => {}
                    _ => panic!("error type should be IllegalArgument"),
                }
            }
            _ => panic!("failure cause should be syscall"),
        },
        _ => panic!("transaction result should have a backtrace"),
    }
}
