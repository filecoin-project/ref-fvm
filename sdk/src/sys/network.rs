// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! Syscalls for network metadata.

// for documentation links
#[doc(inline)]
pub use fvm_shared::sys::out::network::NetworkContext;

#[cfg(doc)]
use crate::sys::ErrorNumber::*;

super::fvm_syscalls! {
    module = "network";

    /// Gets the circulating supply.
    ///
    /// # Errors
    ///
    /// None
    pub fn total_fil_circ_supply() -> Result<super::TokenAmount>;

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

    /// Returns the details about the network.
    ///
    /// # Errors
    ///
    /// None
    pub fn context() -> Result<NetworkContext>;
}
