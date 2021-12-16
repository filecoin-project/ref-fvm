#[link(wasm_import_module = "network")]
extern "C" {
    /// Gets the current epoch.
    pub fn curr_epoch() -> u64;

    /// Gets the network version.
    pub fn version() -> u32;

    /// Gets the base fee for the epoch as little-Endian
    /// tuple of u64 values to be concatenated in a u128.
    #[allow(improper_ctypes)]
    pub fn base_fee() -> (u64, u64);
}
