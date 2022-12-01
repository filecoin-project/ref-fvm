// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

#![no_main]
use fvm_ipld_hamt::Config;
use libfuzzer_sys::fuzz_target;

mod common;

fuzz_target!(|data: (u8, Vec<common::Operation>)| {
    let (flush_rate, operations) = data;
    let mut conf = Config::default();
    conf.bit_width = 5;
    common::run(flush_rate, operations, conf);
});
