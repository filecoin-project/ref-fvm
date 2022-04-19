use fvm_sdk as sdk;

pub unsafe fn do_not_exist(code: u32, message: *const u8, message_len: u32) -> ! {
    extern "C" {
        fn do_not_exist(code: u32, message: *const u8, message_len: u32) -> u32;
    }
    do_not_exist(code, message, message_len);
    {
        std::rt::begin_panic("syscall abort should not have returned")
    }
}
