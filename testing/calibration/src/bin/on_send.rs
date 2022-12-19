// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#![feature(slice_group_by)]

use fil_gas_calibration_actor::{Method, OnSendParams};
use fvm_gas_calibration::*;

const TRANSFER_CHARGE_NAME: &str = "OnValueTransfer";
const INVOKE_CHARGE_NAME: &str = "OnMethodInvocation";
const METHOD: Method = Method::OnSend;

fn main() {
    let iterations = 100;

    let mut te = instantiate_tester();
    let mut invoke_obs = Vec::new();
    let mut transfer_obs = Vec::new();

    for invoke in [true, false] {
        for value_transfer in [true, false] {
            let label = match (invoke, value_transfer) {
                (true, true) => "invoke-and-transfer",
                (false, true) => "transfer-only",
                (true, false) => "invoke-only",
                (false, false) => continue,
            };
            let params = OnSendParams {
                iterations,
                value_transfer,
                invoke,
            };

            let ret = te.execute_or_die(METHOD as u64, &params);

            let both = (value_transfer == invoke) as usize;

            if value_transfer {
                let iter_transfer_obs = collect_obs(&ret, TRANSFER_CHARGE_NAME, label, both);
                let iter_transfer_obs = eliminate_outliers(iter_transfer_obs, 0.02, Eliminate::Top);
                transfer_obs.extend(iter_transfer_obs);
            }

            if invoke {
                let iter_invoke_obs = collect_obs(&ret, INVOKE_CHARGE_NAME, label, both);
                let iter_invoke_obs = eliminate_outliers(iter_invoke_obs, 0.02, Eliminate::Top);
                invoke_obs.extend(iter_invoke_obs);
            }
        }
    }

    let transfer_regs = vec![least_squares("".into(), &transfer_obs, 0)];
    export(TRANSFER_CHARGE_NAME, &transfer_obs, &transfer_regs).unwrap();
    let invoke_regs = vec![least_squares("".into(), &invoke_obs, 0)];
    export(INVOKE_CHARGE_NAME, &invoke_obs, &invoke_regs).unwrap();
}
