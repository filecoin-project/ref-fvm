// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_shared::error::ExitCode;
use fvm_shared::sys::out::vm::MessageContext;

use super::Context;
use super::error::Abort;
use crate::kernel::MessageOps;

/// The maximum message length included in the backtrace. Given 1024 levels, this gives us a total
/// maximum of around 1MiB for debugging.
const MAX_MESSAGE_LEN: usize = 1024;

// NOTE: this won't clobber the last syscall error because it directly returns a "trap".
pub fn exit(
    context: Context<'_, impl Sized>, // "impl Sized" means "any struct"
    code: u32,
    blk: u32,
    message_off: u32,
    message_len: u32,
) -> Abort {
    let code = ExitCode::new(code);
    if !code.is_success() && code.is_system_error() {
        return Abort::Exit(
            ExitCode::SYS_ILLEGAL_EXIT_CODE,
            format!("actor aborted with code {}", code),
            blk,
        );
    }

    let message = if message_len == 0 {
        "actor aborted".to_owned()
    } else {
        match context.memory.try_slice(message_off, message_len) {
            Ok(bytes) => {
                if bytes.len() > MAX_MESSAGE_LEN {
                    let prefix = &bytes[..(MAX_MESSAGE_LEN / 2)];
                    let suffix = &bytes[bytes.len() - (MAX_MESSAGE_LEN / 2)..];
                    format!(
                        "{} ... (skipped {} bytes) ... {}",
                        String::from_utf8_lossy(prefix),
                        bytes.len() - MAX_MESSAGE_LEN,
                        String::from_utf8_lossy(suffix)
                    )
                } else {
                    String::from_utf8_lossy(bytes).into_owned()
                }
            }
            Err(e) => format!("failed to extract error message: {e}"),
        }
    };
    Abort::Exit(code, message, blk)
}

pub fn message_context(
    context: Context<'_, impl MessageOps>,
) -> crate::kernel::Result<MessageContext> {
    context.kernel.msg_context()
}
