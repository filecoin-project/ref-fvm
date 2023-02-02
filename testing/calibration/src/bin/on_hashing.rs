// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#![feature(slice_group_by)]

use fil_gas_calibration_actor::{Method, OnHashingParams};
use fvm_gas_calibration::*;
use fvm_shared::crypto::hash::SupportedHashes;
use rand::{thread_rng, Rng};

const CHARGE_NAME: &str = "OnHashing";
const METHOD: Method = Method::OnHashing;

fn main() {
    let hashers = vec![
        SupportedHashes::Sha2_256,
        SupportedHashes::Blake2b256,
        SupportedHashes::Blake2b512,
        SupportedHashes::Keccak256,
        SupportedHashes::Ripemd160,
    ];

    let sizes = common_sizes();
    let iterations = 100;

    let mut te = instantiate_tester();
    let mut obs = Vec::new();
    let mut rng = thread_rng();

    for hasher in hashers.iter() {
        let label = format!("{hasher:?}");
        for size in sizes.iter() {
            let params = OnHashingParams {
                hasher: *hasher as u64,
                size: *size,
                iterations,
                seed: rng.gen(),
            };

            let ret = te.execute_or_die(METHOD as u64, &params);

            let iter_obs = collect_obs(&ret, CHARGE_NAME, &label, *size);

            // According to the charts there is always an outlier with 10x runtime,
            // which can throw off the model. Maybe it's while some things are warming up.
            // Seems to be present at each call, so once per size. I'll just throw these away.
            let iter_obs = eliminate_outliers(iter_obs, 0.02, Eliminate::Top);

            obs.extend(iter_obs);
        }
    }

    let regs = obs
        .group_by(|a, b| a.label == b.label)
        .map(|g| least_squares(g[0].label.to_owned(), g, 0))
        .collect::<Vec<_>>();

    export(CHARGE_NAME, &obs, &regs).unwrap();
}
