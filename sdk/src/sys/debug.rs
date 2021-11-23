#[link(wasm_import_module = "debug")]
extern "C" {
    pub fn log(level: DebugLevel, message: *mut u8, message_len: u32);
}

#[repr(u8)]
pub enum DebugLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
}
