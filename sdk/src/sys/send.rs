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
    ) -> Result<self::out::Send>;
}

/// Module containing multi-value out types of these syscalls.
pub mod out {
    use crate::ipld::BlockId;

    #[repr(C)]
    pub struct Send {
        pub exit_code: u32,
        pub return_id: BlockId,
    }
}
