//! Syscalls for working with gas.

super::fvm_syscalls! {
    module = "env";

    pub fn timestamp() -> Result<u64>;

    pub fn blockhash(
        block: u8,
        ret_off: *const u8,
        ret_len: u32,
    ) -> Result<u32>;

    pub fn gas_limit() -> Result<u64>;

    pub fn gas_price() -> Result<u64>;
}
