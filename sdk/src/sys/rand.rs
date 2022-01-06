#[link(wasm_import_module = "rand")]
extern "C" {
    /// Gets 32 bytes of randomness from the ticket chain.
    /// The supplied output buffer must have at least 32 bytes of capacity.
    /// If this syscall succeeds, exactly 32 bytes will be written starting at the
    /// supplied offset.
    pub fn get_chain_randomness(
        dst: i64,
        round: i64,
        entropy_offset: *const u8,
        entropy_len: u32,
        obuf: *mut u8,
    ) -> super::SyscallResult0;

    /// Gets 32 bytes of randomness from the beacon system (currently Drand).
    /// The supplied output buffer must have at least 32 bytes of capacity.
    /// If this syscall succeeds, exactly 32 bytes will be written starting at the
    /// supplied offset.
    pub fn get_beacon_randomness(
        dst: i64,
        round: i64,
        entropy_offset: *const u8,
        entropy_len: u32,
        obuf: *mut u8,
    ) -> super::SyscallResult0;
}
