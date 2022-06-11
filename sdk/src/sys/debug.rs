//! Syscalls for debugging.

super::fvm_syscalls! {
    module = "debug";

    /// Returns if we're in debug mode. A zero or positive return value means
    /// yes, a negative return value means no.
    pub fn enabled() -> Result<i32>;

    /// Logs a message on the node.
    pub fn log(message: *const u8, message_len: u32) -> Result<()>;

    /// TODO: this technically lets anyone store whatever data they want on the node's filesystem, this should *not* be enabled on an actual network until proper gaurds are in place
    /// Save data as a debug artifact on the node.
    pub fn capture_artifact(name_off: *const u8, name_len: u32, data_off: *const u8, data_len: u32) -> Result<()>;
}
