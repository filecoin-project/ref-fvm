// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

#![no_main]
use fvm_ipld_hamt::Config;
use libfuzzer_sys::fuzz_target;

mod common;

fuzz_target!(|data: (u8, u32, u32, u32, Vec<common::Operation>)| {
    let (flush_rate, bit_width, min_data_depth, max_array_width, operations) = data;
    let conf = Config {
        bit_width: 1 + bit_width % 8,
        min_data_depth: min_data_depth % 3,
        max_array_width: (max_array_width % 4) as usize, // Starting from 0 just to make sure it doesn't cause an issue.
    };
    common::run(flush_rate, operations, conf);
});
