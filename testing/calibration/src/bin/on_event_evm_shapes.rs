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
    let config: Vec<(usize, (usize, usize))> = vec![
        (1, (2, 32)), // LOG0
        (2, (2, 32)), // LOG1
        (3, (2, 32)), // LOG2
        (4, (2, 32)), // LOG3
        (5, (2, 32)), // LOG4
    ];

    let iterations = 500;

    let (mut validate_obs, mut accept_obs) = (Vec::new(), Vec::new());

    let mut te = instantiate_tester();

    let mut rng = thread_rng();

    for (entries, (key_size, value_size)) in config.iter() {
        let label = format!("{entries:?}entries");
        let params = OnEventParams {
            iterations,
            // number of entries to emit
            entries: *entries,
            mode: EventCalibrationMode::Shape((*key_size, *value_size)),
            flags: Flags::FLAG_INDEXED_ALL,
            seed: rng.gen(),
        };

        let ret = te.execute_or_die(METHOD as u64, &params);

        // projected length of the CBOR payload (confirmed with observations)
        // 1 is the list header; 5 per entry CBOR overhead + flags.
        let len = 1 + entries * value_size + entries * key_size + entries * 5;

        {
            let mut series = collect_obs(&ret.clone(), CHARGE_VALIDATE, &label, len);
            series = eliminate_outliers(series, 0.02, Eliminate::Top);
            validate_obs.extend(series);
        };

        {
            let mut series = collect_obs(&ret.clone(), CHARGE_ACCEPT, &label, len);
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
