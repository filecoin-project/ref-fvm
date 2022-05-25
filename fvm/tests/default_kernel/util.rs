use std::rc::Rc;

use fvm::kernel::BlockRegistry;
use fvm::Kernel;

use super::*;

/// function to reduce a bit of boilerplate
pub fn build_inspecting_test() -> anyhow::Result<(TestingKernel, Rc<RefCell<TestData>>)> {
    // call_manager is not dropped till the end of the function
    let (call_manager, test_data) = dummy::DummyCallManager::new_stub();
    // variable for value inspection, only upgrade after done mutating to avoid panic

    let kern = TestingKernel::new(call_manager, BlockRegistry::default(), 0, 0, 0, 0.into());
    Ok((kern, test_data))
}
