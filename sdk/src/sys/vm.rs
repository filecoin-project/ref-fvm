//! Syscalls for interacting with the VM.

super::fvm_syscalls! {
    module = "vm";

    /// Abort execution with the given code and message. The code is recorded in the receipt, the
    /// message is for debugging only.
    ///
    /// # Errors
    ///
    /// None. This function doesn't return.
    pub fn abort(code: u32, message: *const u8, message_len: u32) -> !;
}
