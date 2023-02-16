// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#![feature(slice_group_by)]

use std::usize;

use fil_gas_calibration_actor::{EventCalibrationMode, Method, OnEventParams};
use fvm_gas_calibration::*;
use fvm_shared::event::Flags;
use rand::{thread_rng, Rng};

const CHARGE_VALIDATE: &str = "OnActorEventValidate";
const CHARGE_ACCEPT: &str = "OnActorEventAccept";
const METHOD: Method = Method::OnEvent;

fn main() {
    // do nothing since we're not interested in these observations at this stage.
}

// Knowingly unused code.
pub fn run() {
    let mut config: Vec<(usize, usize)> = vec![];
    // 1 entry, ranging 8..1024 bytes
    config.extend((3u32..=10).map(|n| (1usize, u64::pow(2, n) as usize)));
    // 2 entry, ranging 16..1024 bytes
    config.extend((4u32..=10).map(|n| (2usize, u64::pow(2, n) as usize)));
    // 4 entries, ranging 32..1024 bytes
    config.extend((5u32..=10).map(|n| (4usize, u64::pow(2, n) as usize)));
    // 8 entries, ranging 64..1024 bytes
    config.extend((6u32..=10).map(|n| (8usize, u64::pow(2, n) as usize)));
    // 16 entries, ranging 128..1024 bytes
    config.extend((7u32..=10).map(|n| (16usize, u64::pow(2, n) as usize)));
    // 32 entries, ranging 256..1024 bytes
    config.extend((8u32..=10).map(|n| (32usize, u64::pow(2, n) as usize)));
    // 64 entries, ranging 512..1024 bytes
    config.extend((9u32..=10).map(|n| (64usize, u64::pow(2, n) as usize)));

    let iterations = 500;

    let (mut validate_obs, mut accept_obs) = (Vec::new(), Vec::new());

    let mut te = instantiate_tester();

    let mut rng = thread_rng();

    for (entries, target_size) in config.iter() {
        let label = format!("{entries:?}entries");
        let params = OnEventParams {
            iterations,
            // number of entries to emit
            entries: *entries,
            // target size of the encoded CBOR; this is approximate.
            mode: EventCalibrationMode::TargetSize(*target_size as usize),
            flags: Flags::FLAG_INDEXED_ALL,
            seed: rng.gen(),
        };

        let ret = te.execute_or_die(METHOD as u64, &params);

        {
            let mut series =
                collect_obs(&ret.clone(), CHARGE_VALIDATE, &label, *target_size as usize);
            series = eliminate_outliers(series, 0.02, Eliminate::Top);
            validate_obs.extend(series);
        };

        {
            let mut series =
                collect_obs(&ret.clone(), CHARGE_ACCEPT, &label, *target_size as usize);
            series = eliminate_outliers(series, 0.02, Eliminate::Top);
            accept_obs.extend(series);
        };
    }

    for (obs, name) in vec![(validate_obs, CHARGE_VALIDATE), (accept_obs, CHARGE_ACCEPT)].iter() {
        let regression = obs
            .group_by(|a, b| a.label == b.label)
            .map(|g| least_squares(g[0].label.to_owned(), g, 0))
            .collect::<Vec<_>>();

        export(name, obs, &regression).unwrap();
    }
}
