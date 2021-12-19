use core::ptr;

pub mod actor;
pub mod gas;
pub mod ipld;
pub mod message;
pub mod network;
pub mod rand;
pub mod sself;
pub mod sys;
pub mod validation;

/// The maximum supported CID size. (SPEC_AUDIT)
pub const MAX_CID_LEN: usize = 100;

/// The maximum actor address length (class 2 addresses).
pub const MAX_ACTOR_ADDR_LEN: usize = 21;

// TODO doesn't work -- fix
#[macro_export]
macro_rules! abort {
    () => { $crate::abort(0, None) };
    ($code:expr) => { $crate::abort($expr, None) };
    ($code:expr, $($rest:expr),+) => {
        let msg = fmt!($(rest),+);
        $crate::abort($expr, Some(&msg));
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

        sys::fvm::abort(code, message, message_len as u32);
    }
}

// TODO: provide a custom panic handler?
