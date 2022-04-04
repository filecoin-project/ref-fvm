use fvm_shared::error::ExitCode;
use fvm_shared::sys::SyscallSafe;
use fvm_shared::version::NetworkVersion;
use num_traits::FromPrimitive;

use super::error::Abort;
use super::Context;
use crate::kernel::{ClassifyResult, Context as _};
use crate::Kernel;

/// An uninhabited type. We use this in `abort` to make sure there's no way to return without
/// returning an error.
#[derive(Copy, Clone)]
pub enum Never {}

unsafe impl SyscallSafe for Never {}

// NOTE: this won't clobber the last syscall error because it directly returns a "trap".
pub fn abort(
    context: Context<'_, impl Kernel>,
    code: u32,
    message_off: u32,
    message_len: u32,
) -> Result<Never, Abort> {
    let code = ExitCode::new(code);
    if context.kernel.network_version() >= NetworkVersion::V16 && code.is_system_error() {
        return Err(Abort::Exit(
            ExitCode::SYS_ILLEGAL_EXIT_CODE,
            format!("actor aborted with code {}", code),
        ));
    }

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
