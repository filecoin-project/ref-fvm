// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use anyhow::anyhow;
use cid::Cid;
use fil_exit_data_actor::WASM_BINARY as EXIT_DATA_BINARY;
use fil_hello_world_actor::WASM_BINARY as HELLO_BINARY;
use fil_ipld_actor::WASM_BINARY as IPLD_BINARY;
use fil_stack_overflow_actor::WASM_BINARY as OVERFLOW_BINARY;
use fil_syscall_actor::WASM_BINARY as SYSCALL_BINARY;
use fvm::executor::{ApplyKind, Executor, ThreadedExecutor};
use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::tester::{Account, IntegrationExecutor};
use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::RawBytes;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::{ErrorNumber, ExitCode};
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use num_traits::Zero;

mod bundles;
use bundles::*;
use fvm_shared::chainid::ChainID;

/// The state object.
#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug, Default)]
pub struct State {
    pub count: u64,
}

#[test]
fn hello_world() {
    // Instantiate tester
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender: [Account; 1] = tester.create_accounts().unwrap();

    let wasm_bin = HELLO_BINARY.unwrap();

    // Set actor state
    let actor_state = State::default();
    let state_cid = tester.set_state(&actor_state).unwrap();

    // Set actor
    let actor_address = Address::new_id(10000);

    tester
        .set_actor_from_bin(wasm_bin, state_cid, actor_address, TokenAmount::zero())
        .unwrap();

    // Instantiate machine
    tester.instantiate_machine(DummyExterns).unwrap();

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
fn ipld() {
    // Instantiate tester
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender: [Account; 1] = tester.create_accounts().unwrap();

    let wasm_bin = IPLD_BINARY.unwrap();

    // Set actor state
    let actor_state = State::default();
    let state_cid = tester.set_state(&actor_state).unwrap();

    // Set actor
    let actor_address = Address::new_id(10000);

    tester
        .set_actor_from_bin(wasm_bin, state_cid, actor_address, TokenAmount::zero())
        .unwrap();

    // Instantiate machine
    tester.instantiate_machine(DummyExterns).unwrap();

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

    if !res.msg_receipt.exit_code.is_success() {
        if let Some(info) = res.failure_info {
            panic!("{}", info)
        } else {
            panic!("non-zero exit code {}", res.msg_receipt.exit_code)
        }
    }
}

#[test]
fn syscalls() {
    // Instantiate tester
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender: [Account; 1] = tester.create_accounts().unwrap();
    tester.set_account_sequence(sender[0].0, 100).unwrap();

    let wasm_bin = SYSCALL_BINARY.unwrap();

    // Set actor state
    let actor_state = State::default();
    let state_cid = tester.set_state(&actor_state).unwrap();

    // Set actor
    let actor_address = Address::new_id(10000);

    tester
        .set_actor_from_bin(wasm_bin, state_cid, actor_address, TokenAmount::zero())
        .unwrap();

    // Instantiate machine
    tester
        .instantiate_machine_with_config(
            DummyExterns,
            |nc| {
                nc.chain_id = ChainID::from(1);
            },
            |_| {},
        )
        .unwrap();

    // Send message
    let message = Message {
        from: sender[0].1,
        to: actor_address,
        gas_limit: 1000000000,
        method_num: 1,
        sequence: 100, // sequence == nonce
        ..Message::default()
    };

    let res = tester
        .executor
        .unwrap()
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    if !res.msg_receipt.exit_code.is_success() {
        if let Some(info) = res.failure_info {
            panic!("{}", info)
        } else {
            panic!("non-zero exit code {}", res.msg_receipt.exit_code)
        }
    }
}

#[test]
fn exit_data() {
    // Instantiate tester
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender: [Account; 1] = tester.create_accounts().unwrap();

    let wasm_bin = EXIT_DATA_BINARY.unwrap();

    // Set actor state
    let actor_state = State::default();
    let state_cid = tester.set_state(&actor_state).unwrap();

    // Set actor
    let actor_address = Address::new_id(10000);

    tester
        .set_actor_from_bin(wasm_bin, state_cid, actor_address, TokenAmount::zero())
        .unwrap();

    // Instantiate machine
    tester.instantiate_machine(DummyExterns).unwrap();

    {
        // Send constructor message
        let message = Message {
            from: sender[0].1,
            to: actor_address,
            gas_limit: 1000000000,
            method_num: 1,
            sequence: 0,
            ..Message::default()
        };

        let res = tester
            .executor
            .as_mut()
            .unwrap()
            .execute_message(message, ApplyKind::Explicit, 100)
            .unwrap();

        assert!(res.msg_receipt.exit_code.is_success());
        assert_eq!(
            res.msg_receipt.return_data,
            RawBytes::from(vec![1u8, 2u8, 3u8, 3u8, 7u8])
        );
    }

    {
        // send method 2
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

        assert!(res.msg_receipt.exit_code.is_success());
        assert_eq!(
            res.msg_receipt.return_data,
            RawBytes::from(vec![1u8, 2u8, 3u8, 3u8, 7u8])
        );
    }

    {
        // send method 3
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

        assert_eq!(res.msg_receipt.exit_code.value(), 0x42);
        assert_eq!(
            res.msg_receipt.return_data,
            RawBytes::from(vec![1u8, 2u8, 3u8, 3u8, 7u8])
        );
    }
}

#[test]
fn native_stack_overflow() {
    // Instantiate tester
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender: [Account; 1] = tester.create_accounts().unwrap();

    let wasm_bin = OVERFLOW_BINARY.unwrap();

    // Set actor state
    let actor_state = State::default();
    let state_cid = tester.set_state(&actor_state).unwrap();

    // Set actor
    let actor_address = Address::new_id(10000);

    tester
        .set_actor_from_bin(wasm_bin, state_cid, actor_address, TokenAmount::zero())
        .unwrap();

    // Instantiate machine
    tester
        .instantiate_machine_with_config(
            DummyExterns,
            |nc| {
                // The stack overflow test consumed the default 512MiB before it hit the recursion limit.
                nc.max_exec_memory_bytes = 4 * (1 << 30);
                nc.max_inst_memory_bytes = 4 * (1 << 30);
            },
            |_| (),
        )
        .unwrap();

    let exec_test =
        |exec: &mut ThreadedExecutor<IntegrationExecutor<MemoryBlockstore, DummyExterns>>,
         method| {
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

            eprintln!("STACKOVERFLOW RESULT = {:?}", res);

            res.msg_receipt.exit_code.value()
        };

    let mut executor = ThreadedExecutor(tester.executor.unwrap());

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

fn test_exitcode(wat: &str, code: ExitCode) {
    // Instantiate tester
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        MemoryBlockstore::default(),
    )
    .unwrap();

    let sender: [Account; 1] = tester.create_accounts().unwrap();

    // Get wasm bin
    let wasm_bin = wat::parse_str(wat).unwrap();

    // Set actor state
    let actor_state = State { count: 0 };
    let state_cid = tester.set_state(&actor_state).unwrap();

    // Set actor
    let actor_address = Address::new_id(10000);

    tester
        .set_actor_from_bin(&wasm_bin, state_cid, actor_address, TokenAmount::zero())
        .unwrap();

    // Instantiate machine
    tester.instantiate_machine(DummyExterns).unwrap();

    // Send message
    let message = Message {
        from: sender[0].1,
        to: actor_address,
        gas_limit: 10_000_000,
        method_num: 1,
        ..Message::default()
    };

    let mut executor = ThreadedExecutor(tester.executor.unwrap());
    let res = executor
        .execute_message(message, ApplyKind::Explicit, 100)
        .unwrap();

    assert_eq!(res.msg_receipt.exit_code, code)
}

#[test]
fn out_of_gas() {
    test_exitcode(
        r#"(module
             (memory (export "memory") 1)
             (func (export "invoke") (param $x i32) (result i32)
               (loop (br 0))
               (i32.const 1)))"#,
        ExitCode::SYS_OUT_OF_GAS,
    )
}

#[test]
fn unreachable() {
    test_exitcode(
        r#"(module
             (memory (export "memory") 1)
             (func (export "invoke") (param $x i32) (result i32)
               unreachable))"#,
        ExitCode::SYS_ILLEGAL_INSTRUCTION,
    );
}

#[test]
fn div_by_zero() {
    test_exitcode(
        r#"(module
             (memory (export "memory") 1)
             (func (export "invoke") (param $x i32) (result i32)
               i32.const 10
               i32.const 0
               i32.div_u))"#,
        ExitCode::SYS_ILLEGAL_INSTRUCTION,
    );
}

#[test]
fn out_of_stack() {
    test_exitcode(
        r#"(module
             (memory (export "memory") 1)
             (func (export "invoke") (param $x i32) (result i32)
               (i64.const 123)
               (call 1)
               (drop)
               (i32.const 0))
             (func (param $x i64) (result i64)
               (local.get 0)
               (call 1)))"#,
        ExitCode::SYS_ILLEGAL_INSTRUCTION,
    );
}

#[test]
fn no_memory() {
    // Make sure we can construct a module with 0 memory pages.
    test_exitcode(
        r#"(module
             (type (;0;) (func (param i32) (result i32)))
             (func (;0;) (type 0) (param i32) (result i32)
               i32.const 0
             )
             (memory (;0;) 0)
             (export "invoke" (func 0))
             (export "memory" (memory 0))
           )
           "#,
        ExitCode::OK,
    );
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
      ;; vm::abort -> vm::exit
      (type (;1;) (func (param i32 i32 i32 i32) (result i32)))
      (import "vm" "exit" (func $fvm_sdk::sys::vm::exit::syscall (type 1)))
      (memory (export "memory") 1)
      (func (export "invoke") (param $x i32) (result i32)
        (i32.const 123)
        (i32.const 123)
        (call $fvm_sdk::sys::ipld::open::syscall)
        (i32.const 0)
        (i32.const 0)
        (i32.const 0)
        (call $fvm_sdk::sys::vm::exit::syscall)
        unreachable
      )
    )
    "#;

    const WAT_FAIL: &str = r#"
    (module
      ;; ipld::open
      (type (;0;) (func (param i32 i32) (result i32)))
      (import "ipld" "open" (func $fvm_sdk::sys::ipld::open::syscall (type 0)))
      ;; vm::abort -> vm::exit
      (type (;1;) (func (param i32 i32 i32 i32) (result i32)))
      (import "vm" "exit" (func $fvm_sdk::sys::vm::exit::syscall (type 1)))
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
        (i32.const 0)
        (call $fvm_sdk::sys::vm::exit::syscall)
        unreachable
      )
    )
    "#;

    let blockstore = {
        let b = FailingBlockstore::default();
        b.add_fail(Cid::try_from("baeaikaia").unwrap());
        Rc::new(b)
    };

    // Instantiate tester
    let mut tester = new_tester(
        NetworkVersion::V18,
        StateTreeVersion::V5,
        blockstore.clone(),
    )
    .unwrap();

    let sender: [Account; 1] = tester.create_accounts().unwrap();

    let state_cid = tester.set_state(&State { count: 0 }).unwrap();

    // Set an actor that aborts.
    let (wasm_abort, wasm_fatal) = (
        wat::parse_str(WAT_ABORT).unwrap(),
        wat::parse_str(WAT_FAIL).unwrap(),
    );
    let (abort_address, fatal_address) = (Address::new_id(10000), Address::new_id(10001));
    tester
        .set_actor_from_bin(&wasm_abort, state_cid, abort_address, TokenAmount::zero())
        .unwrap();
    tester
        .set_actor_from_bin(&wasm_fatal, state_cid, fatal_address, TokenAmount::zero())
        .unwrap();

    // Instantiate machine
    tester.instantiate_machine(DummyExterns).unwrap();

    let executor = tester.executor.as_mut().unwrap();

    let message = Message {
        from: sender[0].1,
        gas_limit: 10_000_000,
        method_num: 1,
        ..Message::default()
    };

    let res = {
        let message = Message {
            to: abort_address,
            ..message.clone()
        };
        executor
            .execute_message(message, ApplyKind::Explicit, 100)
            .unwrap()
    };

    println!("abort backtrace: {}", res.failure_info.unwrap());

    let res = {
        let message = Message {
            to: fatal_address,
            sequence: 1,
            ..message.clone()
        };
        executor
            .execute_message(message, ApplyKind::Explicit, 100)
            .unwrap()
    };

    println!("fatal backtrace: {}", res.failure_info.unwrap());

    // Now make it panic.
    blockstore.panic(true);

    let res = {
        let message = Message {
            to: fatal_address,
            sequence: 2,
            ..message
        };
        executor
            .execute_message(message, ApplyKind::Explicit, 100)
            .unwrap()
    };

    println!("panic backtrace: {}", res.failure_info.unwrap());
}

#[derive(Default)]
pub struct FailingBlockstore {
    fail_for: RefCell<HashSet<Cid>>,
    target: MemoryBlockstore,
    panic: RefCell<bool>,
}

impl FailingBlockstore {
    pub fn add_fail(&self, cid: Cid) {
        self.fail_for.borrow_mut().insert(cid);
    }

    pub fn panic(&self, enabled: bool) {
        *self.panic.borrow_mut() = enabled
    }
}

impl Blockstore for FailingBlockstore {
    fn get(&self, k: &Cid) -> anyhow::Result<Option<Vec<u8>>> {
        if self.fail_for.borrow().contains(k) {
            if *self.panic.borrow() {
                panic!("panic triggered")
            }
            return Err(anyhow!("an error was triggered"));
        }
        self.target.get(k)
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> anyhow::Result<()> {
        self.target.put_keyed(k, block)
    }
}
