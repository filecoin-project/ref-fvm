//! Syscalls for network metadata.

super::fvm_syscalls! {
    module = "network";

    /// Gets the current epoch.
    ///
    /// # Errors
    ///
    /// None
    pub fn curr_epoch() -> Result<i64>;

    /// Gets the network version.
    ///
    /// # Errors
    ///
    /// None
    pub fn version() -> Result<u32>;

    /// Gets the base fee for the current epoch.
    ///
    /// # Errors
    ///
    /// None
    pub fn base_fee() -> Result<super::TokenAmount>;

    /// Gets the circulating supply.
    ///
    /// # Errors
    ///
    /// None
    pub fn total_fil_circ_supply() -> Result<super::TokenAmount>;
}
