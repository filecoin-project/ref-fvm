#[link(wasm_import_module = "send")]
extern "C" {
    /// Sends a message to another actor, and returns the BlockID where
    /// the invocation result (currently a Receipt object) has been placed.
    pub fn send(msg_offset: *const u8, msg_len: u32) -> u32;
}
