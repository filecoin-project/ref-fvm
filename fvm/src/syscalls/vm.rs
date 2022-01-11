use anyhow::Context as _;
use fvm_shared::error::ExitCode;
use num_traits::FromPrimitive;
use wasmtime::Trap;

use super::error::{trap_from_code, trap_from_error};
use super::Context;
use crate::kernel::{ClassifyResult, ExecutionError};
use crate::Kernel;

pub fn abort(
    context: Context<'_, impl Kernel>,
    code: u32,
    message_off: u32,
    message_len: u32,
) -> Result<(), Trap> {
    // Get the error and convert it into a "system illegal argument error" if it's invalid.
    let code = ExitCode::from_u32(code)
        .filter(|c| !c.is_system_error())
        .unwrap_or(ExitCode::SysErrIllegalArgument);

    match (|| {
        let message = if message_len == 0 {
            "actor aborted".to_owned()
        } else {
            std::str::from_utf8(context.memory.try_slice(message_off, message_len)?)
                .context("error message was not utf8")
                .or_illegal_argument()?
                .to_owned()
        };
        context.kernel.push_actor_error(code, message);
        Ok(())
    })() {
        Err(ExecutionError::Syscall(e)) if e.is_recoverable() => {
            // We're logging the actor error here, not the syscall error.
            context.kernel.push_actor_error(
                code,
                format!(
                    "actor aborted with an invalid message: {} (code={:?})",
                    e.0, e.1
                ),
            )
        }
        Err(err) => return Err(trap_from_error(err)),
        Ok(_) => (),
    }

    Err(trap_from_code(code))
}
