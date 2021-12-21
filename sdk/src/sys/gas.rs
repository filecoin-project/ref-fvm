#[link(wasm_import_module = "gas")]
extern "C" {
    // TODO: name for debugging & tracing?
    // We could also _not_ feed that through to the outside?

    /// Charge gas.
    pub fn charge(name_off: *const u8, name_len: u32, amount: u64) -> u32;

    /// Returns the amount of gas remaining.
    /// TODO not implemented.
    /// pub fn remaining() -> (u32, u64);
}
