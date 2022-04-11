//! Syscalls for working with gas.

super::fvm_syscalls! {
    module = "gas";

    // TODO: name for debugging & tracing?
    // We could also _not_ feed that through to the outside?

    /// Charge gas.
    ///
    /// # Errors
    ///
    /// | Error             | Reason               |
    /// |-------------------|----------------------|
    /// | `IllegalArgument` | invalid name buffer. |
    pub fn charge(name_off: *const u8, name_len: u32, amount: u64) -> Result<()>;

    // Returns the amount of gas remaining.
    // TODO not implemented.
    // pub fn remaining() -> Result<u64>;
}
