#[link(wasm_import_module = "network")]
extern "C" {
    pub fn curr_epoch() -> u64;

    pub fn version() -> u32;

    pub fn base_fee(into_off: u32) -> (u64, u64);
}
