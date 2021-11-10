extern "C" {
    pub fn cgobs_get(
        store: i32,
        k: *const u8,
        k_len: i32,
        block: *mut *mut u8,
        size: *mut i32,
    ) -> i32;
    pub fn cgobs_put(store: i32, k: *const u8, k_len: i32, block: *const u8, block_len: i32)
        -> i32;
    pub fn cgobs_delete(store: i32, k: *const u8, k_len: i32) -> i32;
    pub fn cgobs_has(store: i32, k: *const u8, k_len: i32) -> i32;
}
