// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#![feature(slice_group_by)]

use std::collections::HashMap;

use fil_gas_calibration_actor::{Method, OnBlockParams};
use fvm::trace::ExecutionEvent;
use fvm_gas_calibration::*;
use fvm_shared::error::ExitCode;
use rand::{thread_rng, Rng};

fn main() {
    let sizes = common_sizes();
    let iterations = 100;

    let mut all_obs: HashMap<String, Vec<Obs>> = Default::default();

    // NOTE: For actually modeling the effect of IO, we shouldn't be using the memory blockstore.
    // But at the same time when the contracts are executed the changes are buffered in memory,
    // not everything actually gets written to the disk.
    let mut te = instantiate_tester();

    let mut rng = thread_rng();

    // NOTE: The order of sizes (doing them ascending, descending, or shuffled),
    // and whether we reuse the same tester or make a new one for each, does make a difference.

    for size in sizes.iter() {
        let params = OnBlockParams {
            size: *size,
            iterations,
            seed: rng.gen(),
        };

        let ret = te.execute_or_die(Method::OnBlock as u64, &params);

        if let Some(failure) = ret.failure_info {
            panic!("message execution failed: {failure}");
        }
        assert_eq!(ret.msg_receipt.exit_code, ExitCode::OK);

        let mut iter_obs: HashMap<String, Vec<Obs>> = Default::default();

        for event in ret.exec_trace {
            if let ExecutionEvent::GasCharge(charge) = event {
                if charge.name.starts_with("OnBlock") {
                    if let Some(t) = charge.elapsed.get() {
                        let ob = Obs {
                            charge: charge.name.to_string(),
                            label: "n/a".into(),
                            elapsed_nanos: t.as_nanos(),
                            variables: vec![*size],
                            compute_gas: charge.compute_gas.as_milligas(),
                        };
                        iter_obs.entry(charge.name.into()).or_default().push(ob);
                    }
                }
            }
        }
        // The first OnBlockRead is for reading the parameters. From OnBlockStat that's the only record.
        iter_obs.get_mut("OnBlockRead").unwrap().remove(0);
        iter_obs.get_mut("OnBlockStat").unwrap().remove(0);

        for (name, mut obs) in iter_obs {
            if !obs.is_empty() {
                // According to the charts, there are odd outliers.
                obs = eliminate_outliers(obs, 0.02, Eliminate::Top);

                all_obs.entry(name).or_default().extend(obs);
            }
        }
    }

    for (name, obs) in all_obs {
        let regs = vec![least_squares("".into(), &obs, 0)];
        export(&name, &obs, &regs).unwrap();
    }
}
