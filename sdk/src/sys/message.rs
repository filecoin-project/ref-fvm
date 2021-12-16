#[link(wasm_import_module = "message")]
extern "C" {
    /// Returns the caller's actor ID.
    pub fn caller() -> u64;

    /// Returns the receiver's actor ID (i.e. ourselves).
    pub fn receiver() -> u64;

    /// Returns the method number from the message.
    pub fn method_number() -> u64;

    /// Returns the value that was received, as little-Endian
    /// tuple of u64 values to be concatenated in a u128.
    #[allow(improper_ctypes)]
    pub fn value_received() -> (u64, u64);
}
