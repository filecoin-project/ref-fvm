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

/// function to reduce a bit of boilerplate
pub fn build_inspecting_test() -> anyhow::Result<(TestingKernel, Rc<RefCell<TestData>>)> {
    // call_manager is not dropped till the end of the function
    let (call_manager, test_data) = dummy::DummyCallManager::new_stub();
    // variable for value inspection, only upgrade after done mutating to avoid panic

    let kern = TestingKernel::new(call_manager, BlockRegistry::default(), 0, 0, 0, 0.into());
    Ok((kern, test_data))
}

/// function to reduce a bit of boilerplate
pub fn build_inspecting_gas_test(gas_tracker: fvm::gas::GasTracker) -> anyhow::Result<(TestingKernel, Rc<RefCell<TestData>>)> {
    // call_manager is not dropped till the end of the function
    let (call_manager, test_data) = dummy::DummyCallManager::new_with_gas(gas_tracker);
    // variable for value inspection, only upgrade after done mutating to avoid panic

    let kern = TestingKernel::new(call_manager, BlockRegistry::default(), 0, 0, 0, 0.into());
    Ok((kern, test_data))
}