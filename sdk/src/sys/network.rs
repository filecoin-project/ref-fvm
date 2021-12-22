#[link(wasm_import_module = "network")]
#[allow(improper_ctypes)]
extern "C" {
    /// Gets the current epoch.
    pub fn curr_epoch() -> (u32, u64);

    /// Gets the network version.
    pub fn version() -> (u32, u32);

    /// Gets the base fee for the epoch as little-Endian
    /// tuple of u64 values to be concatenated in a u128.
    pub fn base_fee() -> (u32, u64, u64);
}
