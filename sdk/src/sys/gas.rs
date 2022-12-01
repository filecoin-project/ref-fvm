// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! Syscalls for working with gas.

// for documentation links
#[cfg(doc)]
use crate::sys::ErrorNumber::*;

super::fvm_syscalls! {
    module = "gas";

    /// Charge gas.
    ///
    /// # Arguments
    ///
    /// - `name_off` and `name_len` specify the location and length of the "name" of the gas charge,
    ///   for debugging.
    /// - `amount` is the amount of gas to charge.
    ///
    /// # Errors
    ///
    /// | Error               | Reason               |
    /// |---------------------|----------------------|
    /// | [`IllegalArgument`] | invalid name buffer. |
    pub fn charge(name_off: *const u8, name_len: u32, amount: u64) -> Result<()>;

    /// Returns the amount of gas remaining.
    pub fn available() -> Result<u64>;
}
