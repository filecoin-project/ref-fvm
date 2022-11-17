// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

#![no_main]
use fvm_ipld_kamt::Config;
use libfuzzer_sys::fuzz_target;

mod common;

fuzz_target!(|data: (u8, Vec<common::Operation>)| {
    let (flush_rate, operations) = data;
    let conf = Config {
        bit_width: 2,
        use_extensions: true,
        min_data_depth: 1,
        max_array_width: 2,
    };
    common::run(flush_rate, operations, conf);
});
