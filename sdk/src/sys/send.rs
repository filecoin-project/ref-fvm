// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! Syscalls for sending messages to other actors.

#[doc(inline)]
pub use fvm_shared::sys::out::send::*;
#[doc(inline)]
pub use fvm_shared::sys::SendFlags;

// for documentation links
#[cfg(doc)]
use crate::sys::ErrorNumber::*;

super::fvm_syscalls! {
    module = "send";

    /// Sends a message to another actor, and returns the exit code and block ID of the return
    /// result.
    ///
    /// # Arguments
    ///
    /// - `recipient_off` and `recipient_len` specify the location and length of the recipient's
    ///   address (in wasm memory).
    /// - `method` is the method number to invoke.
    /// - `params` is the IPLD block handle of the method parameters.
    /// - `value_hi` are the "high" bits of the token value to send (little-endian) in attoFIL.
    /// - `value_lo` are the "high" bits of the token value to send (little-endian) in attoFIL.
    /// - `gas_limit` is the gas this send is allowed to use. Zero means "all available gas".
    /// - `send_flags` are additional send flags.
    ///
    /// **NOTE**: This syscall will transfer `(value_hi << 64) | (value_lo)` attoFIL to the
    /// recipient.
    ///
    /// # Errors
    ///
    /// A syscall error in [`send`] means the _caller_ did something wrong. If the _callee_ panics,
    /// exceeds some limit, aborts, aborts with an invalid code, etc., the syscall will _succeed_
    /// and the failure will be reflected in the exit code contained in the return value.
    ///
    /// | Error                 | Reason                                               |
    /// |-----------------------|------------------------------------------------------|
    /// | [`NotFound`]          | target actor does not exist and cannot be created.   |
    /// | [`InsufficientFunds`] | tried to send more FIL than available.               |
    /// | [`InvalidHandle`]     | parameters block not found.                          |
    /// | [`LimitExceeded`]     | recursion limit reached.                             |
    /// | [`IllegalArgument`]   | invalid recipient address buffer.                    |
    /// | [`ReadOnly`]          | the send would mutate state in read-only mode.       |
    pub fn send(
        recipient_off: *const u8,
        recipient_len: u32,
        method: u64,
        params: u32,
        value_hi: u64,
        value_lo: u64,
        gas_limit: u64,
        flags: SendFlags,
    ) -> Result<Send>;
}
