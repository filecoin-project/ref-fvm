#![feature(slice_group_by)]

use bls_signatures::Serialize;
use fil_gas_calibration_actor::{Method, OnVerifySignatureParams};
use fvm_gas_calibration::*;
use fvm_shared::address::Address;
use fvm_shared::crypto::signature::SignatureType;
use rand::{thread_rng, Rng};

const CHARGE_NAME: &str = "OnVerifySignature";
const METHOD: Method = Method::OnVerifySignature;

fn main() {
    let sig_types = vec![SignatureType::BLS, SignatureType::Secp256k1];

    let sizes = common_sizes();
    let iterations = 100;

    let mut te = instantiate_tester();
    let mut obs = Vec::new();
    let mut rng = thread_rng();

    for sig_type in sig_types.iter() {
        let label = format!("{sig_type:?}");

        let signer = match sig_type {
            SignatureType::Secp256k1 => {
                let sk = libsecp256k1::SecretKey::random(&mut rng);
                let pk = libsecp256k1::PublicKey::from_secret_key(&sk);
                Address::new_secp256k1(&pk.serialize()).unwrap()
            }
            SignatureType::BLS => {
                let sk = bls_signatures::PrivateKey::generate(&mut rng);
                let pk = sk.public_key();
                Address::new_bls(&pk.as_bytes()).unwrap()
            }
        };

        for size in sizes.iter() {
            let params = OnVerifySignatureParams {
                iterations,
                size: *size,
                signer,
                seed: rng.gen(),
            };

            let ret = te.execute_or_die(METHOD as u64, &params);

            let mut iter_obs = collect_obs(ret, CHARGE_NAME, &label, *size);
            iter_obs = eliminate_outliers(iter_obs, 0.02, Eliminate::Top);

            obs.extend(iter_obs);
        }
    }

    let regs = obs
        .group_by(|a, b| a.label == b.label)
        .map(|g| least_squares(g[0].label.to_owned(), g, 0))
        .collect::<Vec<_>>();

    export(CHARGE_NAME, &obs, &regs).unwrap();
}
