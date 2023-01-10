// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::ptr;

use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::error::ExitCode;

use crate::sys;

/// BlockID representing nil parameters or return data.
pub const NO_DATA_BLOCK_ID: u32 = 0;

/// Returns true if the invocation context is read-only. In read-only mode:
///
/// - State-tree updates `sself::set_root` are forbidden.
/// - Actor creation is forbidden.
/// - Value transfers are forbidden.
/// - Events are discarded.
pub fn read_only() -> bool {
    super::message::MESSAGE_CONTEXT.flags.read_only()
}

/// Abort execution; exit code must be non zero.
pub fn abort(code: u32, message: Option<&str>) -> ! {
    if code == 0 {
        exit(ExitCode::USR_ASSERTION_FAILED.value(), None, message)
    } else {
        exit(code, None, message)
    }
}

/// Exit from current message execution, with the specified code and an optional message and data.
pub fn exit(code: u32, data: Option<IpldBlock>, message: Option<&str>) -> ! {
    unsafe {
        let (message, message_len) = if let Some(m) = message {
            (m.as_ptr(), m.len())
        } else {
            (ptr::null(), 0)
        };

        // Ideally we would write this double if as if let Some(bytes) = data && bytes.len() > 0
        // but the compiler doesn't accept it.
        let blk_id = data.map_or(NO_DATA_BLOCK_ID, |d| {
            sys::ipld::block_create(d.codec, d.data.as_ptr(), d.data.len() as u32).unwrap()
        });

        sys::vm::exit(code, blk_id, message, message_len as u32);
    }
}

/// Sets a panic handler to turn all panics into aborts with `USR_ASSERTION_FAILED`. This should be
/// called early in the actor to improve debuggability.
///
/// NOTE: This will incure a small cost on failure (to format an error message).
pub fn set_panic_handler() {
    std::panic::set_hook(Box::new(|info| {
        abort(
            ExitCode::USR_ASSERTION_FAILED.value(),
            Some(&format!("{}", info)),
        )
    }));
}
