// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use fvm_integration_tests::{tester, testkit};
use fvm_ipld_encoding::BytesDe;
use fvm_shared::address::Address;

pub fn run(
    tester: &mut tester::BasicTester,
    contract: &[u8],
    entrypoint: &[u8],
    params: &[u8],
    gas: u64,
) -> anyhow::Result<()> {
    let mut account = tester.create_basic_account()?;

    let create_res = testkit::fevm::create_contract(tester, &mut account, contract)?;

    if create_res.msg_receipt.exit_code.value() != 0 {
        return Err(anyhow!(
            "actor creation failed: {} -- {:?}",
            create_res.msg_receipt.exit_code,
            create_res.failure_info,
        ));
    }

    let create_return: testkit::fevm::CreateReturn =
        create_res.msg_receipt.return_data.deserialize().unwrap();
    let actor = Address::new_id(create_return.actor_id);

    // invoke contract
    let mut input_data = Vec::from(entrypoint);
    let mut input_params = Vec::from(params);
    input_data.append(&mut input_params);

    let invoke_res = testkit::fevm::invoke_contract(tester, &mut account, actor, &input_data, gas)?;

    if !invoke_res.msg_receipt.exit_code.is_success() {
        return Err(anyhow!(
            "contract invocation failed: {} -- {:?}",
            invoke_res.msg_receipt.exit_code,
            invoke_res.failure_info,
        ));
    }

    let BytesDe(invoke_result) = invoke_res.msg_receipt.return_data.deserialize().unwrap();

    println!("Result: {}", hex::encode(invoke_result));
    println!("Gas Used: {}", invoke_res.msg_receipt.gas_used);

    let options = tester.options.clone().unwrap_or_default();
    if options.trace {
        println!("Execution trace:");
        for tr in invoke_res.exec_trace {
            println!("{:?}", tr)
        }
    }

    if options.events {
        println!("Execution events:");
        for evt in invoke_res.events {
            println!("{:?}", evt)
        }
    }

    Ok(())
}
