#[link(wasm_import_module = "gas")]
extern "C" {
    // TODO: name for debugging & tracing?
    // We could also _not_ feed that through to the outside?

    /// Charge gas.
    pub fn charge(amount: u64);

    /// Returns the amount of gas remaining.
    pub fn remaining() -> u64;
}
