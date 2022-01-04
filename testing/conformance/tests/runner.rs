// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use cid::Cid;
use conformance_tests::externs::TestExterns;
use conformance_tests::vector::{Selector, TestVector};
use conformance_tests::vm::{TestCallManager, TestData, TestKernel, TestMachine};
use fmt::Display;
use fvm::call_manager::DefaultCallManager;
use fvm::executor::{ApplyKind, ApplyRet, DefaultExecutor, Executor};
use fvm::machine::{DefaultMachine, Machine};
use fvm::DefaultKernel;
use fvm_shared::blockstore;
use fvm_shared::encoding::Cbor;
use fvm_shared::message::Message;
use fvm_shared::receipt::Receipt;
use lazy_static::lazy_static;
use regex::Regex;
use std::env::set_var;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use walkdir::{DirEntry, WalkDir};

lazy_static! {
    static ref SKIP_TESTS: Vec<Regex> = vec![
        // currently empty.
    ];
}

/// Checks if the file is a runnable vector.
fn is_runnable(entry: &DirEntry) -> bool {
    let file_name = match entry.path().to_str() {
        Some(file) => file,
        None => return false,
    };

    for rx in SKIP_TESTS.iter() {
        if rx.is_match(file_name) {
            println!("SKIPPING: {}", file_name);
            return false;
        }
    }

    file_name.ends_with(".json")
}

/// Compares the result of running a message with the expected result.
fn check_msg_result(
    expected_rec: &Receipt,
    ret: &ApplyRet,
    label: impl Display,
) -> Result<(), String> {
    let error = ret
        .backtrace
        .iter()
        .map(|e| format!("{:?} {:?} {:?}", e.source, e.code, e.message))
        .collect::<Vec<String>>()
        .join("\n");
    let actual_rec = &ret.msg_receipt;
    let (expected, actual) = (expected_rec.exit_code, actual_rec.exit_code);
    if expected != actual {
        return Err(format!(
            "exit code of msg {} did not match; expected: {:?}, got {:?}. Error: {}",
            label, expected, actual, error
        ));
    }

    let (expected, actual) = (&expected_rec.return_data, &actual_rec.return_data);
    if expected != actual {
        return Err(format!(
            "return data of msg {} did not match; expected: {:?}, got {:?}",
            label,
            expected.as_slice(),
            actual.as_slice()
        ));
    }

    let (expected, actual) = (expected_rec.gas_used, actual_rec.gas_used);
    if expected != actual {
        return Err(format!(
            "gas used of msg {} did not match; expected: {}, got {}",
            label, expected, actual
        ));
    }

    Ok(())
}

/// Compares the resulting state root with the expected state root. Currently,
/// this doesn't do much, but it could run a statediff.
fn compare_state_roots(
    _bs: &blockstore::MemoryBlockstore,
    root: &Cid,
    expected_root: &Cid,
) -> Result<(), String> {
    if root != expected_root {
        let error_msg = format!(
            "wrong post root cid; expected {}, but got {}",
            expected_root, root
        );

        // TODO consider printing a statediff.

        return Err(error_msg.into());
    }
    Ok(())
}

#[async_std::test]
async fn conformance_test_runner() -> Result<(), Box<dyn std::error::Error>> {
    set_var("RUST_LOG", "trace"); // enable debug logs.

    pretty_env_logger::init();

    let walker = WalkDir::new("test-vectors/corpus").into_iter();
    let mut failed = Vec::new();
    let mut succeeded = 0;
    let mut skipped = 0;

    'vectors: for entry in walker.filter_map(|e| e.ok()).filter(is_runnable) {
        let file = File::open(entry.path()).unwrap();
        let reader = BufReader::new(file);
        let test_name = entry.path().display();
        let vector: TestVector = serde_json::from_reader(reader).unwrap();

        match vector {
            TestVector::Message {
                ref selector,
                ref meta,
                ref preconditions,
                ref apply_messages,
                ref postconditions,
                ..
            } => {
                // Skip if selector not supported.
                if !selector.as_ref().map_or(true, Selector::supported) {
                    println!("{} skipped (reason: selector not supported)", test_name);
                    skipped += 1;
                    continue;
                }

                // TODO do we need to care about message-class vectors epoch offset?
                //  All VMs are instantiated at a specific epoch, so it seems weird.
                // if let Some(ep) = m.epoch_offset {
                //     base_epoch += ep;
                // }

                'variants: for variant in &preconditions.variants {
                    // Import the embedded CAR into a memory blockstore.
                    let (mut bs, imported_root) = vector.seed_blockstore().await;
                    assert_eq!(1, imported_root.len());
                    assert_eq!(preconditions.state_tree.root_cid, imported_root[0]);
                    println!("root cid: {}", preconditions.state_tree.root_cid);

                    let machine = TestMachine::new_for_vector(&vector, &variant, bs);
                    let mut exec: DefaultExecutor<
                        TestKernel<DefaultKernel<TestCallManager<DefaultCallManager<_>>>>,
                    > = DefaultExecutor::new(machine);

                    'messages: for (i, m) in apply_messages.iter().enumerate() {
                        let msg = Message::unmarshal_cbor(&m.bytes)?;

                        // TODO: ApplyRet could return the new state root for
                        //  debugging purposes.
                        let ret = exec.execute_message(msg, ApplyKind::Explicit)?;

                        let expected_receipt = &postconditions.receipts[i];

                        // TODO macrofy the checks and success/fail accounting.
                        match check_msg_result(expected_receipt, &ret, i) {
                            Ok(()) => {
                                println!("{} succeeded", test_name);
                                succeeded += 1;
                            }
                            Err(err) => {
                                println!("{} failed, variant {}", test_name, variant.id);
                                failed.push((
                                    format!("{} variant {}", test_name, variant.id),
                                    meta.clone(),
                                    err,
                                ));
                                continue 'variants;
                            }
                        }
                    }

                    // Flush the machine, obtain the blockstore, and compare the
                    // resulting state root with the expected state root.
                    let final_root = match exec.flush() {
                        Ok(cid) => cid,
                        Err(err) => {
                            println!("{} failed, variant {}", test_name, variant.id);
                            failed.push((
                                format!("{} variant {}", test_name, variant.id),
                                meta.clone(),
                                err.to_string(),
                            ));
                            continue 'variants;
                        }
                    };

                    bs = exec.consume().unwrap().consume().consume();

                    // TODO macrofy the checks and success/fail accounting.
                    match compare_state_roots(&bs, &final_root, &postconditions.state_tree.root_cid)
                    {
                        Ok(()) => {
                            println!("{} succeeded", test_name);
                            succeeded += 1;
                        }
                        Err(err) => {
                            println!("{} failed, variant {}", test_name, variant.id);
                            failed.push((
                                format!("{} variant {}", test_name, variant.id),
                                meta.clone(),
                                err,
                            ));
                        }
                    }
                }
            }
        }
    }

    println!(
        "conformance tests result: {}/{} tests passed ({} skipped):",
        succeeded,
        failed.len() + succeeded,
        skipped,
    );
    if !failed.is_empty() {
        for (path, meta, e) in failed {
            eprintln!(
                "file {} failed:\n\tMeta: {:?}\n\tError: {}\n",
                path, meta, e
            );
        }
        panic!()
    }
    Ok(())
}
