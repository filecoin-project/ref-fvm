#[link(wasm_import_module = "send")]
#[allow(improper_ctypes)]
extern "C" {
    /// Sends a message to another actor, and returns the BlockID where
    /// the invocation result (currently a Receipt object) has been placed.
    pub fn send(
        recipient_off: *const u8,
        recipient_len: u32,
        method: u64,
        params: u32,
        value_hi: u64,
        value_lo: u64,
    ) -> (super::SyscallStatus, u32);
}
