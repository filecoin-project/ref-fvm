mod ops;

use std::cell::RefCell;
use std::rc::Rc;

// test target
use fvm::kernel::default::DefaultKernel;
use fvm::kernel::{Block, BlockRegistry};
use fvm::Kernel;
use multihash::Code;

use super::*;

type TestingKernel = DefaultKernel<DummyCallManager>;

/// build a kernel for testing
pub fn build_inspecting_test() -> anyhow::Result<(TestingKernel, Rc<RefCell<TestData>>)> {
    // call_manager is not dropped till the end of the function
    let (call_manager, test_data) = dummy::DummyCallManager::new_stub();
    // variable for value inspection, only upgrade after done mutating to avoid panic

    let kern = TestingKernel::new(call_manager, BlockRegistry::default(), 0, 0, 0, 0.into());
    Ok((kern, test_data))
}

/// build a kernel with a GasTracker
pub fn build_inspecting_gas_test(
    gas_tracker: fvm::gas::GasTracker,
) -> anyhow::Result<(TestingKernel, Rc<RefCell<TestData>>)> {
    // call_manager is not dropped till the end of the function
    let (call_manager, test_data) = dummy::DummyCallManager::new_with_gas(gas_tracker);
    // variable for value inspection, only upgrade after done mutating to avoid panic

    let kern = TestingKernel::new(call_manager, BlockRegistry::default(), 0, 0, 0, 0.into());
    Ok((kern, test_data))
}

#[macro_export]
macro_rules! expect_syscall_err {
    ($code:ident, $res:expr) => {
        match $res.expect_err("expected syscall to fail") {
            ::fvm::kernel::ExecutionError::Syscall(::fvm::kernel::SyscallError(
                _,
                fvm_shared::error::ErrorNumber::$code,
            )) => {}
            ::fvm::kernel::ExecutionError::Syscall(::fvm::kernel::SyscallError(msg, code)) => {
                panic!(
                    "expected {}, got {}: {}",
                    fvm_shared::error::ErrorNumber::$code,
                    code,
                    msg
                )
            }
            ::fvm::kernel::ExecutionError::Fatal(err) => {
                panic!("got unexpected fatal error: {}", err)
            }
            ::fvm::kernel::ExecutionError::OutOfGas => {
                panic!("got unexpected out of gas")
            }
        }
    };
}

#[macro_export]
macro_rules! expect_out_of_gas {
    ($res:expr) => {
        match $res.expect_err("expected syscall to fail") {
            ::fvm::kernel::ExecutionError::OutOfGas => {}
            ::fvm::kernel::ExecutionError::Syscall(::fvm::kernel::SyscallError(msg, code)) => {
                panic!("got unexpected syscall error {}: {}", code, msg)
            }
            ::fvm::kernel::ExecutionError::Fatal(err) => {
                panic!("got unexpected fatal error: {}", err)
            }
        }
    };
}
