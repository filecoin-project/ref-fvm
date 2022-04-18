//! Syscalls for working with gas.

// for documentation links
#[cfg(doc)]
use crate::sys::ErrorNumber::*;

super::fvm_syscalls! {
    module = "gas";

    // TODO: name for debugging & tracing?
    // We could also _not_ feed that through to the outside?

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

    // Returns the amount of gas remaining.
    // TODO not implemented.
    // pub fn remaining() -> Result<u64>;
}
