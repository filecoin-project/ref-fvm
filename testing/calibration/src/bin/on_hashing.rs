#![feature(slice_group_by)]

use fil_gas_calibration_actor::{Method, OnHashingParams};
use fvm::trace::ExecutionEvent;
use fvm_gas_calibration::*;
use fvm_shared::crypto::hash::SupportedHashes;
use rand::{thread_rng, Rng};

fn main() {
    let hashers = vec![
        SupportedHashes::Sha2_256,
        SupportedHashes::Blake2b256,
        SupportedHashes::Blake2b512,
        SupportedHashes::Keccak256,
        SupportedHashes::Ripemd160,
    ];

    let mut sizes: Vec<usize> = vec![0];
    sizes.extend(
        [10, 100, 1_000, 10_000, 100_000]
            .into_iter()
            .flat_map(|i| (1..10).map(move |m| m * i)),
    );
    sizes.push(1_000_000);

    //let sizes: Vec<usize> = (0..=100).map(|i| i * 10000).collect();

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

            let ret = te.execute_or_die(Method::OnHashing as u64, &params);

            let mut iter_obs: Vec<_> = ret
                .exec_trace
                .iter()
                .filter_map(|t| match t {
                    ExecutionEvent::GasCharge(charge) if charge.name == "OnHashing" => Some(Obs {
                        label: label.clone(),
                        elapsed_nanos: charge.elapsed.get().unwrap().as_nanos(),
                        variables: vec![*size],
                    }),
                    _ => None,
                })
                .collect();

            // According to the charts there is always an outlier with 10x runtime,
            // which can throw off the model. Maybe it's while some things are warming up.
            // Seems to be present at each call, so once per size. I'll just throw these away.
            iter_obs = eliminate_outliers(iter_obs, 0.01, Eliminate::Top);

            obs.extend(iter_obs);
        }
    }

    let regs = obs
        .group_by(|a, b| a.label == b.label)
        .map(|g| least_squares(g[0].label.to_owned(), g, 0))
        .collect::<Vec<_>>();

    export("OnHashing", &obs, &regs).unwrap();
}
