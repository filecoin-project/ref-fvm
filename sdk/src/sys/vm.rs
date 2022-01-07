super::fvm_syscalls! {
    module = "vm";

    /// Abort execution with the given code and message. The code is recorded in the receipt, the
    /// message is for debugging only.
    pub fn abort(code: u32, message: *const u8, message_len: u32) -> !;
}
