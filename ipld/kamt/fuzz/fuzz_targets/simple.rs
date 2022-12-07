// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

#![no_main]
use fvm_ipld_kamt::Config;
use libfuzzer_sys::fuzz_target;

mod common;

fuzz_target!(|data: (u8, u32, u32, usize, Vec<common::Operation>)| {
    let (flush_rate, bit_width, min_data_depth, max_array_width, operations) = data;
    let conf = Config {
        bit_width: 1 + bit_width % 8,
        min_data_depth: min_data_depth % 3,
        max_array_width: 1 + max_array_width % 3,
    };
    common::run(flush_rate, operations, conf);
});
