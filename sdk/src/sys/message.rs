//! Syscalls for reading message metadata.

super::fvm_syscalls! {
    module = "message";

    /// Returns the caller's actor ID.
    ///
    /// # Errors
    ///
    /// None
    pub fn caller() -> Result<u64>;

    /// Returns the receiver's actor ID (i.e. ourselves).
    ///
    /// # Errors
    ///
    /// None
    pub fn receiver() -> Result<u64>;

    /// Returns the method number from the message.
    ///
    /// # Errors
    ///
    /// None
    pub fn method_number() -> Result<u64>;

    /// Returns the value that was received.
    ///
    /// # Errors
    ///
    /// None
    pub fn value_received() -> Result<super::TokenAmount>;
}
