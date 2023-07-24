// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! Syscalls related to eventing.

// for documentation links
#[cfg(doc)]
use crate::sys::ErrorNumber::*;

// For documentation
#[doc(inline)]
pub use fvm_shared::sys::EventEntry;

super::fvm_syscalls! {
    module = "event";

    /// Emits an actor event to be recorded in the receipt.
    ///
    /// # Errors
    ///
    /// | Error               | Reason                                                              |
    /// |---------------------|---------------------------------------------------------------------|
    /// | [`IllegalArgument`] | entries failed to validate due to improper encoding or invalid data |
    /// | [`ReadOnly`]        | cannot send events while read-only                                  |
    pub fn emit_event(
        evt_off: *const EventEntry,
        evt_len: u32,
        key_off: *const u8,
        key_len: u32,
        value_off: *const u8,
        value_len: u32,
    ) -> Result<()>;
}
