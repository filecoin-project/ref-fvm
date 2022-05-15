use std::cell::RefCell;
use std::collections::HashSet;
use std::env;

use cid::multihash::Multihash;
use cid::Cid;
use fvm::executor::{ApplyKind, Executor};
use fvm_integration_tests::tester::{Account, IntegrationExecutor, Tester};
use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::DAG_CBOR;
use fvm_shared::address::Address;
use fvm_shared::bigint::BigInt;
use fvm_shared::error::{ErrorNumber, ExitCode};
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use fvm_shared::IDENTITY_HASH;
use num_traits::Zero;
use wabt::wat2wasm;

/// The state object.
#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug, Default)]
pub struct State {
    pub count: u64,
}

const WASM_COMPILED_PATH: &str =
    "../../target/debug/wbuild/fil_hello_world_actor/fil_hello_world_actor.compact.wasm";

const WASM_COMPILED_PATH_OVERFLOW: &str =
    "../../target/debug/wbuild/fil_stack_overflow_actor/fil_stack_overflow_actor.compact.wasm";

#[test]
fn hello_world() {
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
        .unwrap()
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    assert_eq!(res.msg_receipt.exit_code.value(), 16)
}

#[test]
fn native_stack_overflow() {
    // Instantiate tester
    let mut tester = Tester::new(
        NetworkVersion::V16,
        StateTreeVersion::V4,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender: [Account; 1] = tester.create_accounts().unwrap();

    // Get wasm bin
    let wasm_path = env::current_dir()
        .unwrap()
        .join(WASM_COMPILED_PATH_OVERFLOW)
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

    let exec_test = |exec: &mut IntegrationExecutor<MemoryBlockstore>, method| {
        // Send message
        let message = Message {
            from: sender[0].1,
            to: actor_address,
            gas_limit: 10_000_000_000,
            method_num: method,
            sequence: method - 1,
            ..Message::default()
        };

        let res = exec
            .execute_message(message, ApplyKind::Explicit, 100)
            .unwrap();

        res.msg_receipt.exit_code.value()
    };

    let mut executor = tester.executor.unwrap();

    // on method 0 the test actor should run out of stack
    assert_eq!(
        exec_test(&mut executor, 1),
        ExitCode::SYS_ILLEGAL_INSTRUCTION.value()
    );

    // on method 1 the test actor should run out of recursive call limit
    assert_eq!(
        exec_test(&mut executor, 2),
        0xc0000000 + (ErrorNumber::LimitExceeded as u32)
    );

    // on method 2 the test actor should finish successfully
    assert_eq!(exec_test(&mut executor, 3), 0x80000042);
}

#[test]
fn out_of_gas() {
    const WAT: &str = r#"
    ;; Mock invoke function
    (module
      (memory (export "memory") 1)
      (func (export "invoke") (param $x i32) (result i32)
        (loop
            (br 0)
        )
        (i32.const 1)
      )
    )
    "#;

    // Instantiate tester
    let mut tester = Tester::new(
        NetworkVersion::V16,
        StateTreeVersion::V4,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender: [Account; 1] = tester.create_accounts().unwrap();

    // Get wasm bin
    let wasm_bin = wat2wasm(WAT).unwrap();

    // Set actor state
    let actor_state = State { count: 0 };
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

    let res = tester
        .executor
        .unwrap()
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    assert_eq!(res.msg_receipt.exit_code, ExitCode::SYS_OUT_OF_GAS)
}

#[test]
fn out_of_stack() {
    const WAT: &str = r#"
    ;; Mock invoke function
    (module
      (memory (export "memory") 1)
      (func (export "invoke") (param $x i32) (result i32)
        (i64.const 123)
        (call 1)
        (drop)
        (i32.const 0)
      )
      (func (param $x i64) (result i64)
        (local.get 0)
        (call 1)
      )
    )
    "#;

    // Instantiate tester
    let mut tester = Tester::new(
        NetworkVersion::V16,
        StateTreeVersion::V4,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender: [Account; 1] = tester.create_accounts().unwrap();

    // Get wasm bin
    let wasm_bin = wat2wasm(WAT).unwrap();

    // Set actor state
    let actor_state = State { count: 0 };
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

    let res = tester
        .executor
        .unwrap()
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    assert_eq!(res.msg_receipt.exit_code, ExitCode::SYS_ILLEGAL_INSTRUCTION)
}

#[test]
fn backtraces() {
    // Note: this test **does not actually assert anything**, but it's useful to
    // let us peep into FVM backtrace generation under different scenarios.
    const WAT_ABORT: &str = r#"
    (module
      ;; ipld::open
      (type (;0;) (func (param i32 i32) (result i32)))
      (import "ipld" "open" (func $fvm_sdk::sys::ipld::open::syscall (type 0)))
      ;; vm::abort
      (type (;1;) (func (param i32 i32 i32) (result i32)))
      (import "vm" "abort" (func $fvm_sdk::sys::vm::abort::syscall (type 1)))
      (memory (export "memory") 1)
      (func (export "invoke") (param $x i32) (result i32)
        (i32.const 123)
        (i32.const 123)
        (call $fvm_sdk::sys::ipld::open::syscall)
        (i32.const 0)
        (i32.const 0)
        (call $fvm_sdk::sys::vm::abort::syscall)
        unreachable
      )
    )
    "#;

    const WAT_FATAL: &str = r#"
    (module
      ;; ipld::open
      (type (;0;) (func (param i32 i32) (result i32)))
      (import "ipld" "open" (func $fvm_sdk::sys::ipld::open::syscall (type 0)))
      ;; vm::abort
      (type (;1;) (func (param i32 i32 i32) (result i32)))
      (import "vm" "abort" (func $fvm_sdk::sys::vm::abort::syscall (type 1)))
      (memory (export "memory") 1)
      (func (export "invoke") (param $x i32) (result i32)
        (i32.const 128)
        (memory.grow)
        (i32.const 4)
        (i32.const 25493505)
        (i32.store)
        (i32.const 8)
        (i32.const 0)
        (i32.store)
        (i32.const 4)
        (call $fvm_sdk::sys::ipld::open::syscall)
        (i32.const 0)
        (i32.const 0)
        (call $fvm_sdk::sys::vm::abort::syscall)
        unreachable
      )
    )
    "#;

    let blockstore = FailingBlockstore::default();
    let identity_cid = Cid::new_v1(DAG_CBOR, Multihash::wrap(IDENTITY_HASH, &[0]).unwrap());
    blockstore.add_fail(identity_cid);

    // Instantiate tester
    let mut tester = Tester::new(
        NetworkVersion::V16,
        StateTreeVersion::V4,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender: [Account; 1] = tester.create_accounts().unwrap();

    let state_cid = tester.set_state(&State { count: 0 }).unwrap();

    // Set an actor that aborts.
    let (wasm_abort, wasm_fatal) = (wat2wasm(WAT_ABORT).unwrap(), wat2wasm(WAT_FATAL).unwrap());
    let (abort_address, fatal_address) = (Address::new_id(10000), Address::new_id(10001));
    tester
        .set_actor_from_bin(&wasm_abort, state_cid, abort_address, BigInt::zero())
        .unwrap();
    tester
        .set_actor_from_bin(&wasm_fatal, state_cid, fatal_address, BigInt::zero())
        .unwrap();

    // Instantiate machine
    tester.instantiate_machine().unwrap();

    // Send message
    let message = Message {
        from: sender[0].1,
        to: abort_address,
        gas_limit: 10_000_000,
        method_num: 1,
        ..Message::default()
    };

    let res = tester
        .executor
        .as_mut()
        .unwrap()
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    println!("abort backtrace: {}", res.failure_info.unwrap());

    // Send message
    let message = Message {
        from: sender[0].1,
        to: fatal_address,
        gas_limit: 10_000_000,
        method_num: 1,
        sequence: 1,
        ..Message::default()
    };

    let res = tester
        .executor
        .as_mut()
        .unwrap()
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    println!("fatal backtrace: {}", res.failure_info.unwrap());
}

#[derive(Default)]
pub struct FailingBlockstore {
    fail_for: RefCell<HashSet<Cid>>,
    target: MemoryBlockstore,
}

impl FailingBlockstore {
    pub fn add_fail(&self, cid: Cid) {
        self.fail_for.borrow_mut().insert(cid);
    }
}

impl Blockstore for FailingBlockstore {
    fn get(&self, k: &Cid) -> anyhow::Result<Option<Vec<u8>>> {
        self.target.get(k)
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> anyhow::Result<()> {
        self.target.put_keyed(k, block)
    }
}
