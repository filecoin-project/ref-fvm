// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#![feature(slice_group_by)]

use fil_gas_calibration_actor::{Method, OnRecoverSecpPublicKeyParams};
use fvm_gas_calibration::*;
use rand::{thread_rng, Rng, RngCore};

const CHARGE_NAME: &str = "OnRecoverSecpPublicKey";
const METHOD: Method = Method::OnRecoverSecpPublicKey;

fn main() {
    // Just doing it for uniformity.
    let sizes = common_sizes();
    let iterations = 10;

    let mut te = instantiate_tester();
    let mut obs = Vec::new();
    let mut rng = thread_rng();

    // Generate a signature over some data to ensure it's not complete rubbish.
    let mut data = vec![0u8; 100];
    rng.fill_bytes(&mut data);

    let sk = libsecp256k1::SecretKey::random(&mut rng);
    let sig = secp_sign(&sk, &data);

    for size in sizes.iter() {
        let params = OnRecoverSecpPublicKeyParams {
            iterations,
            size: *size,
            signature: sig.to_vec(),
            seed: rng.gen(),
        };

        let ret = te.execute_or_die(METHOD as u64, &params);

        let iter_obs = collect_obs(&ret, CHARGE_NAME, "n/a", *size);
        //let iter_obs = eliminate_outliers(iter_obs, 0.02, Eliminate::Top);

        obs.extend(iter_obs);
    }

    let regs = vec![least_squares("".into(), &obs, 0)];

    export(CHARGE_NAME, &obs, &regs).unwrap();
}
