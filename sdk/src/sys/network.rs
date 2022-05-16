//! Syscalls for network metadata.

super::fvm_syscalls! {
    module = "network";

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
