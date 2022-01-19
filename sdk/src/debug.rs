use lazy_static::lazy_static;

use crate::sys;

lazy_static! {
    /// Lazily memoizes if debug mode is enabled.
    static ref DEBUG_ENABLED: bool = unsafe { sys::debug::enabled().unwrap() >= 0 };
}

/// Logs a message on the node.
#[inline]
pub fn log(msg: String) {
    unsafe {
        sys::debug::log(msg.as_ptr(), msg.len() as u32).unwrap();
    }
}

/// Returns whether debug mode is enabled.
#[inline(always)]
fn enabled() -> bool {
    *DEBUG_ENABLED
}

/// Logger is a debug-only logger that uses the FVM syscalls.
pub struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        // TODO: per-level?
        enabled()
    }

    fn log(&self, record: &log::Record) {
        if enabled() {
            log(format!("[{}] {}", record.level(), record.args()));
        }
    }

    fn flush(&self) {}
}

/// Initialize logging if debuggig is enabled.
pub fn init_logging() {
    if enabled() {
        log::set_logger(&Logger).expect("failed to enable logging");
    }
}
