use super::Context;
use crate::kernel::Result;
use crate::Kernel;

pub fn on_submit_verify_seal(context: Context<'_, impl Kernel>) -> Result<()> {
    context.kernel.on_submit_verify_seal()
}
