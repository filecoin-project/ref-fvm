//! Syscalls for sending messages to other actors.

#[doc(inline)]
pub use fvm_shared::sys::out::send::*;

super::fvm_syscalls! {
    module = "send";

    /// Sends a message to another actor, and returns the exit code and block ID of the return
    /// result.
    ///
    /// # Errors
    ///
    /// A syscall error in [`send`] means the _caller_ did something wrong. If the _callee_ panics,
    /// exceeds some limit, aborts, aborts with an invalid code, etc., the syscall will _succeed_
    /// and the failure will be reflected in the exit code contained in the return value.
    ///
    /// | Error               | Reason                                               |
    /// |---------------------|------------------------------------------------------|
    /// | `NotFound`          | target actor does not exist and cannot be created.   |
    /// | `InsufficientFunds` | tried to send more FIL than available.               |
    /// | `InvalidHandle`     | parameters block not found.                          |
    /// | `LimitExceeded`     | recursion limit reached.                             |
    /// | `IllegalArgument`   | invalid recipient address buffer.                    |
    pub fn send(
        recipient_off: *const u8,
        recipient_len: u32,
        method: u64,
        params: u32,
        value_hi: u64,
        value_lo: u64,
    ) -> Result<Send>;
}
