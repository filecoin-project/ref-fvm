use fvm::kernel::BlockRegistry;
use fvm::Kernel;

use super::*;

/// function to reduce a bit of boilerplate
pub fn build_inspecting_test() -> anyhow::Result<(TestingKernel, ExternalCallManager)> {
    // call_manager is not dropped till the end of the function
    let call_manager = dummy::DummyCallManager::new_stub();
    // variable for value inspection, only upgrade after done mutating to avoid panic
    let refcell = call_manager.weak();

    let kern = TestingKernel::new(call_manager, BlockRegistry::default(), 0, 0, 0, 0.into());
    Ok((kern, refcell))
}
