//! Syscalls for debugging.

super::fvm_syscalls! {
    module = "debug";

    /// Returns if we're in debug mode. A zero or positive return value means
    /// yes, a negative return value means no.
    pub fn enabled() -> Result<i32>;

    /// Logs a message on the node.
    pub fn log(message: *const u8, message_len: u32) -> Result<()>;
}
