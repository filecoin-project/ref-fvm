use crate::sys;
use lazy_static::lazy_static;

lazy_static! {
    /// Lazily memoizes if debug mode is enabled.
    pub static ref DEBUG_ENABLED: bool = enabled();
}

/// Logs a message on the node.
#[inline]
pub fn log(msg: String) {
    unsafe {
        sys::debug::log(msg.as_ptr(), msg.len() as u32).unwrap();
    }
}

/// Returns whether debug mode is enabled.
#[inline]
pub fn enabled() -> bool {
    unsafe { sys::debug::enabled().unwrap() >= 0 }
}
