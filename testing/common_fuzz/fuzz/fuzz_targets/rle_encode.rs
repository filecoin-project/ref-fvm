// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#![no_main]
use arbitrary::Arbitrary;
use fvm_ipld_bitfield::BitField;
use libfuzzer_sys::fuzz_target;

#[derive(Debug, Arbitrary)]
enum Operation {
    Set(u64),
    Unset(u64),
}

fuzz_target!(|data: (BitField, Vec<Operation>)| {
    let (mut bf, ops) = data;

    for op in ops {
        match op {
            Operation::Set(x) => {
                let _ = bf.try_set(x);
            }
            Operation::Unset(x) => {
                bf.unset(x);
            }
        };
    }

    let bf_bytes = bf.to_bytes();
    let bf2 = BitField::from_bytes(&bf_bytes).unwrap();
    assert_eq!(bf, bf2);

    let bf2_bytes = bf.to_bytes();
    assert_eq!(bf_bytes, bf2_bytes);
});
