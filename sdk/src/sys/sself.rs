#[link(wasm_import_module = "self")]
#[allow(improper_ctypes)]
extern "C" {
    /// Gets the current root for the calling actor.
    ///
    /// If the CID doesn't fit in the specified maximum length (and/or the length is 0), this
    /// function returns the required size and does not update the cid buffer.
    pub fn root(cid: *mut u8, cid_max_len: u32) -> (super::SyscallStatus, u32);

    /// Sets the root CID for the calling actor. The new root must be in the reachable set.
    pub fn set_root(cid: *const u8) -> super::SyscallStatus;

    /// Gets the current balance for the calling actor.
    pub fn current_balance() -> (super::SyscallStatus, u64, u64);

    /// Destroys the calling actor, sending its current balance
    /// to the supplied address, which cannot be itself.
    pub fn self_destruct(addr_off: *const u8, addr_len: u32) -> super::SyscallStatus;
}
