//! Syscalls for working with gas.

super::fvm_syscalls! {
    module = "env";

    pub fn tipset_timestamp() -> Result<u64>;

    pub fn tipset_cid(
        epoch: i64,
        ret_off: *mut u8,
        ret_len: u32,
    ) -> Result<u32>;
}
