// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#![feature(slice_group_by)]

use bls_signatures::Serialize;
use fil_gas_calibration_actor::{Method, OnVerifySignatureParams};
use fvm_gas_calibration::*;
use fvm_shared::address::Address;
use fvm_shared::crypto::signature::SignatureType;
use rand::{thread_rng, Rng, RngCore};

const CHARGE_NAME: &str = "OnVerifySignature";
const METHOD: Method = Method::OnVerifySignature;

fn main() {
    let sig_types = vec![SignatureType::BLS, SignatureType::Secp256k1];

    let sizes = common_sizes();
    let iterations = 100;

    let mut te = instantiate_tester();
    let mut obs = Vec::new();
    let mut rng = thread_rng();

    // Just some random data over which we can generate an example signature.
    // Having a valid BLS signature is important otherwise verification is
    // an instant rejection without hasing the input data.
    let mut data = vec![0u8; 100];
    rng.fill_bytes(&mut data);

    for sig_type in sig_types.iter() {
        let label = format!("{sig_type:?}");

        let (signer, signature) = match sig_type {
            SignatureType::Secp256k1 => {
                let sk = libsecp256k1::SecretKey::random(&mut rng);
                let pk = libsecp256k1::PublicKey::from_secret_key(&sk);
                let addr = Address::new_secp256k1(&pk.serialize()).unwrap();
                let sig = secp_sign(&sk, &data).into();
                (addr, sig)
            }
            SignatureType::BLS => {
                let sk = bls_signatures::PrivateKey::generate(&mut rng);
                let pk = sk.public_key();
                let addr = Address::new_bls(&pk.as_bytes()).unwrap();
                let sig = sk.sign(&data).as_bytes();
                (addr, sig)
            }
        };

        for size in sizes.iter() {
            let params = OnVerifySignatureParams {
                iterations,
                size: *size,
                signer,
                signature: signature.clone(),
                seed: rng.gen(),
            };

            let ret = te.execute_or_die(METHOD as u64, &params);

            let iter_obs = collect_obs(&ret, CHARGE_NAME, &label, *size);
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
