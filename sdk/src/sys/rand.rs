//! Syscalls for getting randomness.

use fvm_shared::randomness::RANDOMNESS_LENGTH;

super::fvm_syscalls! {
    module = "rand";

    /// Gets 32 bytes of randomness from the ticket chain.
    /// The supplied output buffer must have at least 32 bytes of capacity.
    /// If this syscall succeeds, exactly 32 bytes will be written starting at the
    /// supplied offset.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                  |
    /// |-------------------|-------------------------|
    /// | `LimitExceeded`   | lookback exceeds limit. |
    /// | `IllegalArgument` | invalid buffer, etc.    |
    pub fn get_chain_randomness(
        dst: i64,
        round: i64,
        entropy_offset: *const u8,
        entropy_len: u32,
    ) -> Result<[u8; RANDOMNESS_LENGTH]>;

    /// Gets 32 bytes of randomness from the beacon system (currently Drand).
    /// The supplied output buffer must have at least 32 bytes of capacity.
    /// If this syscall succeeds, exactly 32 bytes will be written starting at the
    /// supplied offset.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                  |
    /// |-------------------|-------------------------|
    /// | `LimitExceeded`   | lookback exceeds limit. |
    /// | `IllegalArgument` | invalid buffer, etc.    |
    pub fn get_beacon_randomness(
        dst: i64,
        round: i64,
        entropy_offset: *const u8,
        entropy_len: u32,
    ) -> Result<[u8; RANDOMNESS_LENGTH]>;
}
