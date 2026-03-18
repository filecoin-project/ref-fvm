// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! Syscalls for getting randomness.

use fvm_shared::randomness::RANDOMNESS_LENGTH;

// for documentation links
#[cfg(doc)]
use crate::sys::ErrorNumber::*;

super::fvm_syscalls! {
    module = "rand";

    /// Gets 32 bytes of randomness from the ticket chain,
    /// there is no lookback limit.
    ///
    /// # Arguments
    ///
    /// - `epoch` is the epoch to pull the randomness from.
    ///
    /// # Errors
    ///
    /// | Error               | Reason                 |
    /// |---------------------|------------------------|
    /// | [`IllegalArgument`] | epoch is in the future |
    pub fn get_chain_randomness(
        epoch: i64,
    ) -> Result<[u8; RANDOMNESS_LENGTH]>;

    /// Gets 32 bytes of randomness from the beacon system
    /// (currently Drand), there is no lookback limit.
    ///
    /// # Arguments
    ///
    /// - `epoch` is the epoch to pull the randomness from.
    ///
    /// # Errors
    ///
    /// | Error               | Reason                 |
    /// |---------------------|------------------------|
    /// | [`IllegalArgument`] | epoch is in the future |
    pub fn get_beacon_randomness(
        epoch: i64,
    ) -> Result<[u8; RANDOMNESS_LENGTH]>;
}
