//! Syscalls for network metadata.

// for documentation links
#[cfg(doc)]
use crate::sys::ErrorNumber::*;

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
    pub fn tipset_timestamp() -> Result<i64>;

    /// Retrieves a tipset's CID within the last finality, if available
    ///
    /// # Arguments
    ///
    /// - `epoch` the epoch being queried.
    /// - `ret_off` and `ret_len` specify the location and length of the buffer into which the
    ///   tipset CID will be written.
    ///
    /// # Returns
    ///
    /// Returns the length of the CID written to the output buffer.
    ///
    /// # Errors
    ///
    /// | Error               | Reason                                       |
    /// |---------------------|----------------------------------------------|
    /// | [`IllegalArgument`] | specified epoch is negative or in the future |
    /// | [`LimitExceeded`]   | specified epoch exceeds finality             |
    pub fn tipset_cid(
        epoch: i64,
        ret_off: *mut u8,
        ret_len: u32,
    ) -> Result<u32>;
}
