#[link(wasm_import_module = "message")]
#[allow(improper_ctypes)]
extern "C" {
    /// Returns the caller's actor ID.
    pub fn caller() -> (super::SyscallStatus, u64);

    /// Returns the receiver's actor ID (i.e. ourselves).
    pub fn receiver() -> (super::SyscallStatus, u64);

    /// Returns the method number from the message.
    pub fn method_number() -> (super::SyscallStatus, u64);

    /// Returns the value that was received, as little-Endian
    /// tuple of u64 values to be concatenated in a u128.
    pub fn value_received() -> (super::SyscallStatus, u64, u64);
}
