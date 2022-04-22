//! Syscalls for reading message metadata.

use fvm_shared::sys::out::message::MessageDetails;

super::fvm_syscalls! {
    module = "message";

    /// Returns the details about this invocation.
    ///
    /// - The caller's actor ID.
    /// - The receiver's actor ID (i.e. ourselves).
    /// - The method number from the message.
    /// - The value that was received.
    /// - The current epoch.
    /// - The network version.
    /// - The base fee for the current epoch.
    /// - The circulating supply.
    ///
    /// # Errors
    ///
    /// None
    pub fn details() -> Result<MessageDetails>;
}
