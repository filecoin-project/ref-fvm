use std::ptr;

use fvm_ipld_encoding::{RawBytes, DAG_CBOR};
use fvm_shared::error::ExitCode;

use crate::sys;

/// BlockID representing nil parameters or return data.
pub const NO_DATA_BLOCK_ID: u32 = 0;

/// Abort execution; exit code must be non zero.
pub fn abort(code: u32, message: Option<&str>) -> ! {
    if code == 0 {
        exit(
            ExitCode::USR_ASSERTION_FAILED.value(),
            RawBytes::default(),
            message,
        )
    } else {
        exit(code, RawBytes::default(), message)
    }
}

/// Exit from current message execution, with the specified code and an optional message and data.
pub fn exit(code: u32, data: RawBytes, message: Option<&str>) -> ! {
    unsafe {
        let (message, message_len) = if let Some(m) = message {
            (m.as_ptr(), m.len())
        } else {
            (ptr::null(), 0)
        };

        // Ideally we would write this double if as if let Some(bytes) = data && bytes.len() > 0
        // but the compiler doexn't accept it.
        let blk_id = if data.len() > 0 {
            sys::ipld::block_create(DAG_CBOR, data.as_ptr(), data.len() as u32).unwrap()
        } else {
            NO_DATA_BLOCK_ID
        };

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
