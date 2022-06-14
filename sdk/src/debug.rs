pub use inner::*;

#[cfg(not(feature = "debug"))]
mod inner {
    #[inline(always)]
    pub fn init_logging() {}

    #[inline(always)]
    pub fn enabled() {
        false
    }
    #[inline(always)]
    pub fn log(_: String) {}
}

#[cfg(feature = "debug")]
mod inner {
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
    /// Initialize logging if debugging is enabled.
    pub fn init_logging() {
        if enabled() {
            log::set_logger(&Logger).expect("failed to enable logging");
        }
    }

    /// Saves an artifact to the host env. New artifacts with the same name will overwrite old ones
    pub fn store_artifact(name: impl AsRef<str>, data: impl AsRef<[u8]>) {
        let name = name.as_ref();
        let data = data.as_ref();
        unsafe {
            sys::debug::store_artifact(
                name.as_ptr(),
                name.len() as u32,
                data.as_ptr(),
                data.len() as u32,
            )
            .unwrap();
        }
    }

    /// Returns whether debug mode is enabled.
    #[inline(always)]
    pub fn enabled() -> bool {
        *DEBUG_ENABLED
    }

    /// Logger is a debug-only logger that uses the FVM syscalls.
    struct Logger;

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
}
