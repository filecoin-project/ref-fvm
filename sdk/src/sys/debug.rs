super::fvm_syscalls! {
    module = "debug";

    pub fn log(/*level: DebugLevel, */message: *const u8, message_len: u32) -> Result<()>;
}
//
// #[repr(u8)]
// pub enum DebugLevel {
//     Off,
//     Error,
//     Warn,
//     Info,
//     Debug,
// }
