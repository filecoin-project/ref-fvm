#![feature(slice_group_by)]

use fil_gas_calibration_actor::{HashingParams, Method};
use fvm::executor::{ApplyKind, Executor};
use fvm::trace::ExecutionEvent;
use fvm_gas_calibration::*;
use fvm_ipld_encoding::RawBytes;
use fvm_shared::crypto::hash::SupportedHashes;
use fvm_shared::error::ExitCode;
use fvm_shared::message::Message;

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
    let mut sequence = 0;

    for hasher in hashers.iter() {
        let label = format!("{hasher:?}");
        for size in sizes.iter() {
            let params = HashingParams {
                hasher: *hasher as u64,
                size: *size,
                iterations,
            };

            let raw_params = RawBytes::serialize(&params).unwrap();

            let message = Message {
                from: te.sender.1,
                to: te.actor_address,
                sequence,
                gas_limit: ENOUGH_GAS.as_milligas(),
                method_num: Method::Hashing as u64,
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

            obs.extend(ret.exec_trace.iter().filter_map(|t| match t {
                ExecutionEvent::GasCharge(charge) if charge.name == "OnHashing" => Some(Obs {
                    label: label.clone(),
                    elapsed_nanos: charge.elapsed.get().unwrap().as_nanos(),
                    variables: vec![*size],
                }),
                _ => None,
            }));
        }
    }

    let regs = obs
        .group_by(|a, b| a.label == b.label)
        .map(|g| least_squares(g[0].label.to_owned(), g, 0))
        .collect::<Vec<_>>();

    export("OnHashing", &obs, &regs).unwrap();
}