// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! Syscalls related to eventing.

// for documentation links
#[cfg(doc)]
use crate::sys::ErrorNumber::*;

super::fvm_syscalls! {
    module = "event";

    /// Emits an actor event to be recorded in the receipt.
    ///
    /// Expects a DAG-CBOR representation of the ActorEvent struct.
    ///
    /// # Errors
    ///
    /// | Error               | Reason                                                              |
    /// |---------------------|---------------------------------------------------------------------|
    /// | [`IllegalArgument`] | entries failed to validate due to improper encoding or invalid data |
    pub fn emit_event(
        evt_off: *const u8,
        evt_len: u32,
    ) -> Result<()>;
}
