use crate::ipld::BlockId;

super::fvm_syscalls! {
    module = "send";

    /// Sends a message to another actor, and returns the exit code and block ID of the return
    /// result.
    pub fn send(
        recipient_off: *const u8,
        recipient_len: u32,
        method: u64,
        params: u32,
        value_hi: u64,
        value_lo: u64,
    ) -> Result<(u32, BlockId)>;
}
