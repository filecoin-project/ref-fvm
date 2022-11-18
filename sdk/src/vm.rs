use std::ptr;

use fvm_ipld_encoding::{RawBytes, DAG_CBOR};
use fvm_shared::error::ExitCode;

use crate::sys;

/// BlockID representing nil parameters or return data.
pub const NO_DATA_BLOCK_ID: u32 = 0;

/// Abort execution.
pub fn abort(code: u32, message: Option<&str>) -> ! {
    exit(code, message, None)
}

/// Exit from current message execution, with the specified code and an optional message and data.
pub fn exit(code: u32, message: Option<&str>, data: Option<RawBytes>) -> ! {
    unsafe {
        let (message, message_len) = if let Some(m) = message {
            (m.as_ptr(), m.len())
        } else {
            (ptr::null(), 0)
        };

        // Ideally we would write this double if as if let Some(bytes) = data && bytes.len() > 0
        // but the compiler doexn't accept it.
        let blk_id = if let Some(bytes) = data {
            if bytes.len() > 0 {
                sys::ipld::block_create(DAG_CBOR, bytes.as_ptr(), bytes.len() as u32).unwrap()
            } else {
                NO_DATA_BLOCK_ID
            }
        } else {
            NO_DATA_BLOCK_ID
        };

        sys::vm::exit(code, message, message_len as u32, blk_id);
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
