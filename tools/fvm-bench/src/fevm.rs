// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use fvm::executor::ApplyRet;
use fvm_integration_tests::{tester, testkit};
use fvm_ipld_encoding::BytesDe;
use fvm_shared::address::Address;

fn handle_result(tester: &tester::BasicTester, name: &str, res: &ApplyRet) -> anyhow::Result<()> {
    let (trace, events) = tester
        .options
        .as_ref()
        .map(|o| (o.trace, o.events))
        .unwrap_or_default();

    if trace && !res.exec_trace.is_empty() {
        println!();
        println!("**");
        println!("* BEGIN {name} execution trace");
        println!("**");
        println!();
        for tr in &res.exec_trace {
            println!("{:?}", tr)
        }
        println!();
        println!("**");
        println!("* END {name} execution trace");
        println!("**");
        println!();
    }
    if events && !res.events.is_empty() {
        println!();
        println!("**");
        println!("* BEGIN {name} events");
        println!("**");
        println!();
        for evt in &res.events {
            println!("{:?}", evt)
        }
        println!();
        println!("**");
        println!("* END {name} events");
        println!("**");
        println!();
    }

    if let Some(bt) = &res.failure_info {
        println!("{bt}");
    }

    if res.msg_receipt.exit_code.is_success() {
        Ok(())
    } else {
        Err(anyhow!("{name} failed"))
    }
}

pub fn run(
    tester: &mut tester::BasicTester,
    contract: &[u8],
    entrypoint: &[u8],
    params: &[u8],
    gas: u64,
) -> anyhow::Result<()> {
    let mut account = tester.create_basic_account()?;

    let create_res = testkit::fevm::create_contract(tester, &mut account, contract)?;
    handle_result(tester, "contract creation", &create_res)?;

    let create_return: testkit::fevm::CreateReturn =
        create_res.msg_receipt.return_data.deserialize().unwrap();
    let actor = Address::new_id(create_return.actor_id);

    // invoke contract
    let mut input_data = Vec::from(entrypoint);
    let mut input_params = Vec::from(params);
    input_data.append(&mut input_params);

    let invoke_res = testkit::fevm::invoke_contract(tester, &mut account, actor, &input_data, gas)?;
    let BytesDe(returnval) = invoke_res.msg_receipt.return_data.deserialize().unwrap();
    println!("Exit Code: {}", invoke_res.msg_receipt.exit_code);
    println!("Result: {}", hex::encode(returnval));
    println!("Gas Used: {}", invoke_res.msg_receipt.gas_used);

    handle_result(tester, "contract invocation", &invoke_res)
}
