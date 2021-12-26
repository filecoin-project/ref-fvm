#[link(wasm_import_module = "network")]
#[allow(improper_ctypes)]
extern "C" {
    /// Gets the current epoch.
    pub fn curr_epoch() -> (super::SyscallStatus, u64);

    /// Gets the network version.
    pub fn version() -> (super::SyscallStatus, u32);

    /// Gets the base fee for the epoch as little-Endian
    /// tuple of u64 values to be concatenated in a u128.
    pub fn base_fee() -> (super::SyscallStatus, u64, u64);

    /// Gets the circulating supply as little-Endian
    /// tuple of u64 values to be concatenated in a u128.
    /// Note that how this value is calculated is expected to change in nv15
    pub fn total_fil_circ_supply() -> (super::SyscallStatus, u64, u64);
}
