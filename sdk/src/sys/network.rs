super::fvm_syscalls! {
    module = "network";

    /// Gets the current epoch.
    pub fn curr_epoch() -> Result<i64>;

    /// Gets the network version.
    pub fn version() -> Result<u32>;

    /// Gets the base fee for the epoch as little-Endian
    /// tuple of u64 values to be concatenated in a u128.
    pub fn base_fee() -> Result<fvm_shared::sys::TokenAmount>;

    /// Gets the circulating supply as little-Endian
    /// tuple of u64 values to be concatenated in a u128.
    /// Note that how this value is calculated is expected to change in nv15
    pub fn total_fil_circ_supply() -> Result<fvm_shared::sys::TokenAmount>;
}
