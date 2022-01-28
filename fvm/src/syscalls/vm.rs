use fvm_shared::error::ExitCode;
use num_traits::FromPrimitive;

use super::error::Abort;
use super::Context;
use crate::kernel::{ClassifyResult, Context as _};
use crate::Kernel;

// NOTE: this won't clobber the last syscall error because it directly returns a "trap".
pub fn abort(
    context: Context<'_, impl Kernel>,
    code: u32,
    message_off: u32,
    message_len: u32,
) -> Result<(), Abort> {
    // Get the error and convert it into a "system illegal argument error" if it's invalid.
    // BUG: https://github.com/filecoin-project/fvm/issues/253
    let code = ExitCode::from_u32(code)
        //.filter(|c| !c.is_system_error())
        .unwrap_or(ExitCode::SysErrIllegalActor); // TODO: will become "illegal exit"

    let message = if message_len == 0 {
        "actor aborted".to_owned()
    } else {
        std::str::from_utf8(
            context
                .memory
                .try_slice(message_off, message_len)
                .map_err(|e| Abort::from_error(code, e))?,
        )
        .or_illegal_argument()
        .context("error message was not utf8")
        .map_err(|e| Abort::from_error(code, e))?
        .to_owned()
    };
    Err(Abort::Exit(code, message))
}
