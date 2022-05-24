// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

#![no_main]

use fvm_ipld_bitfield::BitField;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bfs: [BitField; 3]| {
    let [bf1, bf2, bf3] = &bfs;
    {
        assert_eq!(bf1 | bf2, bf2 | bf1);
        assert_eq!(bf1 | bf3, bf3 | bf1);
        assert_eq!(bf2 | bf3, bf3 | bf2);

        assert_eq!(bf1 & bf2, bf2 & bf1);
        assert_eq!(bf1 & bf3, bf3 & bf1);
        assert_eq!(bf2 & bf3, bf3 & bf2);

        assert_eq!(bf1 ^ bf2, bf2 ^ bf1);
        assert_eq!(bf1 ^ bf3, bf3 ^ bf1);
        assert_eq!(bf2 ^ bf3, bf3 ^ bf2);

        assert_eq!(bf1 - bf2, bf1 ^ &(bf1 & bf2));
        assert_eq!(bf1 ^ bf2, &(bf1 | bf2) - &(bf1 & bf2));

        assert_eq!(bf1 ^ bf1, BitField::new());
        assert_eq!(bf1.cut(bf1), BitField::new());
    }
    {
        assert_eq!(&(bf1 - bf2) - bf3, &(bf1 - bf3) - bf2);
        assert_eq!(&(bf1 ^ bf2) ^ bf3, &(bf1 ^ bf3) ^ bf2);
    }
});
