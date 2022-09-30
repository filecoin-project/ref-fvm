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

    /// Gets the current tipset's timestamp
    ///
    /// # Errors
    ///
    /// None
    pub fn tipset_timestamp() -> Result<u64>;

    /// Retrieves a tipset's CID within the last finality, if available
    ///
    /// # Errors
    ///
    /// IllegalArgument -- raised when the epoch is negative or greater/equal than finality.
    pub fn tipset_cid(
        epoch: i64,
        ret_off: *mut u8,
        ret_len: u32,
    ) -> Result<u32>;
}
