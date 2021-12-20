use num_traits::FromPrimitive;

use anyhow::Context as _;
use fvm_shared::error::ExitCode;
use wasmtime::{Caller, Trap};

use crate::{
    kernel::{ClassifyResult, ExecutionError},
    Kernel,
};

use super::{
    error::{trap_from_code, trap_from_error},
    Context as _,
};

pub fn abort(
    caller: &mut Caller<'_, impl Kernel>,
    code: u32,
    message_off: u32,
    message_len: u32,
) -> Result<(), Trap> {
    if message_len != 0 {
        match (|| {
            let (_kernel, memory) = caller.kernel_and_memory()?;
            let _message = std::str::from_utf8(memory.try_slice(message_off, message_len)?)
                .context("error message was not utf8")
                .or_illegal_argument()?;
            // Log the message here...
            Ok(())
        })() {
            Err(ExecutionError::Syscall(_e)) => {
                // TODO: record that we failed to read the message.
                // But don't return an error. The only way out of this function is an abort.
            }
            Err(ExecutionError::Fatal(e)) => return Err(trap_from_error(e)),
            Ok(_) => (),
        }
    }

    // TODO:
    // 1. Figure out the right fallback. Probably wants to be some "illegal error" error?
    // 2. Check to make sure the actor is can only raise actor errors.
    Err(trap_from_code(
        ExitCode::from_u32(code).unwrap_or(ExitCode::SysErrIllegalArgument),
    ))
}
