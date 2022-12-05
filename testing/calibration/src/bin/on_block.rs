#![feature(slice_group_by)]

use std::collections::HashMap;

use fil_gas_calibration_actor::{Method, OnBlockParams};
use fvm::executor::{ApplyKind, Executor};
use fvm::trace::ExecutionEvent;
use fvm_gas_calibration::*;
use fvm_ipld_encoding::RawBytes;
use fvm_shared::error::ExitCode;
use fvm_shared::message::Message;

fn main() {
    let mut sizes: Vec<usize> = vec![0];
    sizes.extend(
        [10, 100, 1_000, 10_000, 100_000]
            .into_iter()
            .flat_map(|i| (1..10).map(move |m| m * i)),
    );
    sizes.push(1_000_000);

    let iterations = 100;
    //let keep = (iterations as f32 * 0.95) as usize;

    let mut te = instantiate_tester();
    let mut sequence = 0;
    let mut obs: HashMap<String, Vec<Obs>> = Default::default();

    for size in sizes.iter() {
        let params = OnBlockParams {
            size: *size,
            iterations,
        };

        let raw_params = RawBytes::serialize(&params).unwrap();

        let message = Message {
            from: te.sender.1,
            to: te.actor_address,
            sequence,
            gas_limit: ENOUGH_GAS.as_milligas(),
            method_num: Method::OnBlock as u64,
            params: raw_params,
            ..Message::default()
        };
        sequence += 1;

        let ret = te
            .tester
            .executor
            .as_mut()
            .unwrap()
            .execute_message(message, ApplyKind::Explicit, 100)
            .unwrap();

        if let Some(failure) = ret.failure_info {
            panic!("message execution failed: {failure}");
        }
        assert_eq!(ret.msg_receipt.exit_code, ExitCode::OK);

        for event in ret.exec_trace {
            if let ExecutionEvent::GasCharge(charge) = event {
                // OnBlockStat is only called once per call to read the params. It's not what we're measuring.
                if charge.name.starts_with("OnBlock") && charge.name != "OnBlockStat" {
                    if let Some(t) = charge.elapsed.get() {
                        let o = Obs {
                            label: "n/a".into(),
                            elapsed_nanos: t.as_nanos(),
                            variables: vec![*size],
                        };
                        obs.entry(charge.name.into()).or_default().push(o);
                    }
                }
            }
        }
    }

    for (name, obs) in obs {
        let regs = vec![least_squares("".into(), &obs, 0)];
        export(&name, &obs, &regs).unwrap();
    }
}
