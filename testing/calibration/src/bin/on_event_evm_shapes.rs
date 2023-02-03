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
    let entries = 1..=5;
    let (key_size, value_size) = (2, 32); // 2 bytes per key, 32 bytes per value (topics)
    let last_entry_value_sizes = (5u32..=13).map(|n| u64::pow(2, n) as usize); // 32 bytes to 8KiB (payload)

    let iterations = 500;

    let (mut validate_obs, mut accept_obs) = (Vec::new(), Vec::new());

    let mut te = instantiate_tester();

    let mut rng = thread_rng();

    for entry_count in entries {
        for last_entry_value_size in last_entry_value_sizes.clone() {
            let label = format!("{entry_count:?}entries");
            let params = OnEventParams {
                iterations,
                // number of entries to emit
                entries: entry_count,
                mode: EventCalibrationMode::Shape((key_size, value_size, last_entry_value_size)),
                flags: Flags::FLAG_INDEXED_ALL,
                seed: rng.gen(),
            };

            let ret = te.execute_or_die(METHOD as u64, &params);

            // Estimated length of the CBOR payload (confirmed with observations)
            // 1 is the list header; 5 per entry CBOR overhead + flags.
            let len = 1
                + ((entry_count - 1) * value_size)
                + last_entry_value_size
                + entry_count * key_size
                + entry_count * 5;

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
    }

    for (obs, name) in vec![(validate_obs, CHARGE_VALIDATE), (accept_obs, CHARGE_ACCEPT)].iter() {
        let regression = obs
            .group_by(|a, b| a.label == b.label)
            .map(|g| least_squares(g[0].label.to_owned(), g, 0))
            .collect::<Vec<_>>();

        export(name, obs, &regression).unwrap();
    }
}
