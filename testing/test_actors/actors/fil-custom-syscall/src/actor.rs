// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
fvm_sdk::sys::fvm_syscalls! {
    module = "my_custom_kernel";
    pub fn my_custom_syscall(doubleme: i32) -> Result<i32>;
}

#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    fvm_sdk::initialize();

    unsafe {
        let value = my_custom_syscall(11).unwrap();
        assert_eq!(value, 22, "expected 22, got {}", value);
    }

    0
}
