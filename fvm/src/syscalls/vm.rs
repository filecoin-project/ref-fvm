use fvm_shared::error::ExitCode;
use fvm_shared::sys::out::vm::InvocationContext;
use fvm_shared::sys::SyscallSafe;
use fvm_shared::version::NetworkVersion;

use super::error::Abort;
use super::Context;
use crate::kernel::{ClassifyResult, Kernel};

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
    use crate::kernel::Context as _;

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

pub fn context(context: Context<'_, impl Kernel>) -> crate::kernel::Result<InvocationContext> {
    use anyhow::Context as _;

    Ok(InvocationContext {
        caller: context.kernel.msg_caller(),
        receiver: context.kernel.msg_receiver(),
        method_number: context.kernel.msg_method_number(),
        value_received: context
            .kernel
            .msg_value_received()
            .try_into()
            .context("invalid token amount")
            .or_fatal()?,
        network_curr_epoch: context.kernel.network_epoch(),
        network_version: context.kernel.network_version() as u32,
    })
}
