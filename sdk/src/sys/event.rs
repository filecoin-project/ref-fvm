//! Syscalls related to eventing.

// for documentation links
#[cfg(doc)]
use crate::sys::ErrorNumber::*;

super::fvm_syscalls! {
    module = "event";

    /// Records an actor event. Expect a DAG-CBOR representation of the event.
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
