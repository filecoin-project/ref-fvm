use std::ptr;

use fvm_shared::error::ExitCode;

use crate::sys;

/// BlockID representing nil parameters or return data.
pub const NO_DATA_BLOCK_ID: u32 = 0;

/// Abort execution.
pub fn abort(code: u32, message: Option<&str>) -> ! {
    unsafe {
        let (message, message_len) = if let Some(m) = message {
            (m.as_ptr(), m.len())
        } else {
            (ptr::null(), 0)
        };

        sys::vm::abort(code, message, message_len as u32);
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
