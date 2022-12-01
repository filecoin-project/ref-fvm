// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#![no_main]
use fvm_ipld_bitfield::BitField;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let bf = BitField::from_bytes(data);
    if bf.is_err() {
        return;
    }

    let recreated = bf.unwrap().to_bytes();
    assert_eq!(&recreated, data);
});
