//! Syscalls for getting randomness.

use fvm_shared::randomness::RANDOMNESS_LENGTH;

// for documentation links
#[cfg(doc)]
use crate::sys::ErrorNumber::*;

super::fvm_syscalls! {
    module = "rand";

    /// Gets 32 bytes of randomness from the ticket chain.
    ///
    /// # Arguments
    ///
    /// - `tag` is the "domain separation tag" for distinguishing between different categories of
    ///    randomness. Think of it like extra, structured entropy.
    /// - `epoch` is the epoch to pull the randomness from.
    /// - `entropy_off` and `entropy_len` specify the location and length of the entropy buffer that
    ///    will be mixed into the system randomness.
    ///
    /// # Errors
    ///
    /// | Error               | Reason                  |
    /// |---------------------|-------------------------|
    /// | [`LimitExceeded`]   | lookback exceeds limit. |
    /// | [`IllegalArgument`] | invalid buffer, etc.    |
    pub fn get_chain_randomness(
        tag: i64,
        epoch: i64,
        entropy_off: *const u8,
        entropy_len: u32,
    ) -> Result<[u8; RANDOMNESS_LENGTH]>;

    /// Gets 32 bytes of randomness from the beacon system (currently Drand).
    ///
    /// # Arguments
    ///
    /// - `tag` is the "domain separation tag" for distinguishing between different categories of
    ///    randomness. Think of it like extra, structured entropy.
    /// - `epoch` is the epoch to pull the randomness from.
    /// - `entropy_off` and `entropy_len` specify the location and length of the entropy buffer that
    ///    will be mixed into the system randomness.
    ///
    /// # Errors
    ///
    /// | Error               | Reason                  |
    /// |---------------------|-------------------------|
    /// | [`LimitExceeded`]   | lookback exceeds limit. |
    /// | [`IllegalArgument`] | invalid buffer, etc.    |
    pub fn get_beacon_randomness(
        tag: i64,
        epoch: i64,
        entropy_off: *const u8,
        entropy_len: u32,
    ) -> Result<[u8; RANDOMNESS_LENGTH]>;
}
