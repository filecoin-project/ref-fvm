use std::ptr;

use fvm_shared::sys::out::vm::InvocationContext;

use crate::sys;

/// BlockID representing nil parameters or return data.
pub const NO_DATA_BLOCK_ID: u32 = 0;

lazy_static::lazy_static! {
    pub(crate) static ref INVOCATION_CONTEXT: InvocationContext = {
        unsafe {
            sys::vm::context().expect("failed to lookup invocation context")
        }
    };
}

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
