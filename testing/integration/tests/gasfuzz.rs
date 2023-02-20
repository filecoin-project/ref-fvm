// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

mod bundles;

use std::fs;

use anyhow::Context;
use fvm::trace::{ExecutionEvent, ExecutionTrace};
use fvm_integration_tests::{tester, testkit};
use fvm_shared::address::Address;
use fvm_shared::error::ExitCode;

const CONTRACT_PATH: &str = "../../tools/contracts/gas-stress/recursive.bin";

#[test]
fn test_gasfuzz() {
    // get all charge points we want to fuzz at
    let trace = gasfuzz_get_exec_trace();

    let mut charge_points_milligas = Vec::new();
    let mut aggregate_charge = 0u64;
    for tr in trace.iter() {
        if let ExecutionEvent::GasCharge(ch) = tr {
            let this_charge = ch.total();
            aggregate_charge += this_charge.as_milligas();
            charge_points_milligas.push(aggregate_charge);
        }
    }

    gasfuzz_fuzz(charge_points_milligas);
}

fn gasfuzz_fuzz(charge_points_milligas: Vec<u64>) {
    // set up the tester
    let options = tester::ExecutionOptions {
        debug: false,
        trace: false,
        events: false,
    };

    let mut tester = bundles::new_basic_tester(options).unwrap();
    let mut account = tester.create_basic_account().unwrap();
    let contract = hex::decode(fs::read_to_string(CONTRACT_PATH).unwrap())
        .context("error decoding contract")
        .unwrap();

    // create the contract
    let create_res = testkit::fevm::create_contract(&mut tester, &mut account, &contract).unwrap();
    assert!(create_res.msg_receipt.exit_code.is_success());

    let create_return: testkit::fevm::CreateReturn =
        create_res.msg_receipt.return_data.deserialize().unwrap();
    let actor = Address::new_id(create_return.actor_id);

    println!(
        "Fuzzing gas for {} charge points",
        charge_points_milligas.len()
    );
    // invoke contract at every charge point +/- 1 gas.; we should still  error with OutOfGas
    // skip the first chage, as that results in SYS_SENDER_STATE_INVALID
    for cpm in charge_points_milligas[1..].iter() {
        println!("Fuzzing gas at {}", cpm / 1000);

        let gas_lo = (cpm - 500) / 1000;
        let invoke_res =
            testkit::fevm::invoke_contract(&mut tester, &mut account, actor, &[], gas_lo).unwrap();
        assert_eq!(invoke_res.msg_receipt.exit_code, ExitCode::SYS_OUT_OF_GAS);

        let gas_hi = (cpm + 500) / 1000;
        let invoke_res =
            testkit::fevm::invoke_contract(&mut tester, &mut account, actor, &[], gas_hi).unwrap();
        assert_eq!(invoke_res.msg_receipt.exit_code, ExitCode::SYS_OUT_OF_GAS);
    }
}

fn gasfuzz_get_exec_trace() -> ExecutionTrace {
    let options = tester::ExecutionOptions {
        debug: false,
        trace: true,
        events: false,
    };

    let mut tester = bundles::new_basic_tester(options).unwrap();
    let mut account = tester.create_basic_account().unwrap();
    let contract = hex::decode(fs::read_to_string(CONTRACT_PATH).unwrap())
        .context("error decoding contract")
        .unwrap();

    let create_res = testkit::fevm::create_contract(&mut tester, &mut account, &contract).unwrap();
    assert!(create_res.msg_receipt.exit_code.is_success());

    let create_return: testkit::fevm::CreateReturn =
        create_res.msg_receipt.return_data.deserialize().unwrap();
    let actor = Address::new_id(create_return.actor_id);

    // this number is not arbitrary.
    // contract recurses if gas > 10M, and empty contract run takes a tad less than 2M.
    // So upon execution the contract shoud have just enough for 1 recursive call.
    let gas = 12_000_000;
    let invoke_res =
        testkit::fevm::invoke_contract(&mut tester, &mut account, actor, &[], gas).unwrap();
    assert_eq!(invoke_res.msg_receipt.exit_code, ExitCode::SYS_OUT_OF_GAS);

    invoke_res.exec_trace
}
