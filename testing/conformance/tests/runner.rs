// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::{anyhow, Result};
use cid::Cid;
use conformance_tests::vector::{MessageVector, Selector, TestVector, Variant};
use conformance_tests::vm::{TestCallManager, TestKernel, TestMachine};
use fmt::Display;
use fvm::call_manager::DefaultCallManager;
use fvm::executor::{ApplyKind, ApplyRet, DefaultExecutor, Executor};
use fvm::machine::Machine;
use fvm::DefaultKernel;
use fvm_shared::blockstore;
use fvm_shared::encoding::Cbor;
use fvm_shared::message::Message;
use fvm_shared::receipt::Receipt;
use lazy_static::lazy_static;
use regex::Regex;
use std::env::var;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
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
fn check_msg_result(expected_rec: &Receipt, ret: &ApplyRet, label: impl Display) -> Result<()> {
    let error = ret
        .backtrace
        .iter()
        .map(|e| format!("{:?} {:?} {:?}", e.source, e.code, e.message))
        .collect::<Vec<String>>()
        .join("\n");
    let actual_rec = &ret.msg_receipt;
    let (expected, actual) = (expected_rec.exit_code, actual_rec.exit_code);
    if expected != actual {
        return Err(anyhow!(
            "exit code of msg {} did not match; expected: {:?}, got {:?}. Error: {}",
            label,
            expected,
            actual,
            error
        ));
    }

    let (expected, actual) = (&expected_rec.return_data, &actual_rec.return_data);
    if expected != actual {
        return Err(anyhow!(
            "return data of msg {} did not match; expected: {:?}, got {:?}",
            label,
            expected.as_slice(),
            actual.as_slice()
        ));
    }

    let (expected, actual) = (expected_rec.gas_used, actual_rec.gas_used);
    if expected != actual {
        return Err(anyhow!(
            "gas used of msg {} did not match; expected: {}, got {}",
            label,
            expected,
            actual
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
) -> Result<()> {
    if root != expected_root {
        // TODO consider printing a statediff.

        return Err(anyhow!(
            "wrong post root cid; expected {}, but got {}",
            expected_root,
            root
        ));
    }
    Ok(())
}

#[async_std::test]
async fn conformance_test_runner() -> Result<(), Box<dyn std::error::Error>> {
    pretty_env_logger::init();

    let vector_results: Vec<(PathBuf, Vec<VariantResult>)> = match var("VECTOR") {
        Ok(v) => {
            let path = Path::new(v.as_str()).to_path_buf();
            let res = run_vector(&path).await;
            vec![(path, res)]
        }
        Err(_) => {
            let paths: Vec<PathBuf> = WalkDir::new("test-vectors/corpus")
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(is_runnable)
                .map(|e| e.path().to_path_buf())
                .collect();

            let mut ret = Vec::new();
            for path in paths {
                // Cannot use iterator map here because of the async function.
                let res = run_vector(&path).await;
                ret.push((path, res));
            }
            ret
        }
    };

    let mut succeeded = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for (_, ress) in vector_results {
        for res in ress {
            match res {
                VariantResult::Ok { .. } => succeeded += 1,
                VariantResult::Failed { .. } => failed += 1,
                VariantResult::Skipped { .. } => skipped += 1,
            }
        }
    }

    println!(
        "conformance tests result: {}/{} tests passed ({} skipped):",
        succeeded,
        failed + succeeded,
        skipped,
    );

    if failed > 0 {
        Err(String::from("some vectors failed").into())
    } else {
        Ok(())
    }
}

/// Represents the result from running a vector.
enum VariantResult {
    /// The vector succeeded.
    Ok { id: String },
    /// A variant was skipped, due to the specified reason.
    Skipped { reason: String, id: String },
    /// A variant failed, due to the specified error.
    Failed { reason: anyhow::Error, id: String },
}

/// Runs a single test vector and returns a list of VectorResults,
/// one per variant.
async fn run_vector(path: &PathBuf) -> Vec<VariantResult> {
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);
    let vector: TestVector = serde_json::from_reader(reader).unwrap();

    match vector {
        TestVector::Message(v) => {
            let variants = &v.preconditions.variants;
            let skip = !v.selector.as_ref().map_or(true, Selector::supported);
            let results = if skip {
                variants
                    .iter()
                    .map(|variant| VariantResult::Skipped {
                        id: variant.id.clone(),
                        reason: "selector not supported".to_owned(),
                    })
                    .collect()
            } else {
                let mut ret = Vec::with_capacity(variants.len());
                for variant in variants.iter() {
                    // Cannot use a functional approach without dealing with
                    // futures streams due to the async :-(
                    ret.push(run_variant(&v, &variant).await);
                }
                ret
            };

            // Output the result to stdout.
            // Doing this here instead of in an inspect so that we get streaming output.
            for res in &results {
                match &res {
                    VariantResult::Ok { id } => {
                        println!("OK vector {}, variant {}", path.display(), id);
                    }
                    VariantResult::Failed { reason, id } => {
                        println!(
                            "FAIL vector {}, variant {}, reason: {:?}",
                            path.display(),
                            id,
                            reason
                        );
                    }
                    VariantResult::Skipped { reason, id } => {
                        println!(
                            "SKIP vector {}, variant {}, reason: {:?}",
                            path.display(),
                            id,
                            reason
                        );
                    }
                }
            }

            results
        }
    }
}

async fn run_variant(v: &MessageVector, variant: &Variant) -> VariantResult {
    let id = variant.id.clone();

    // Import the embedded CAR into a memory blockstore.
    let (mut bs, imported_root) = v.seed_blockstore().await;

    // Sanity checks.
    if imported_root.len() != 1 {
        return VariantResult::Failed {
            id,
            reason: anyhow!("expected one root; found {}", imported_root.len()),
        };
    }
    if v.preconditions.state_tree.root_cid != imported_root[0] {
        return VariantResult::Failed {
            id,
            reason: anyhow!(
                "imported root does not match precondition root; imported: {}; expected: {}",
                imported_root[0],
                v.preconditions.state_tree.root_cid
            ),
        };
    }

    // Construct the Machine.
    let machine = TestMachine::new_for_vector(&v, &variant, bs);
    let mut exec: DefaultExecutor<
        TestKernel<DefaultKernel<TestCallManager<DefaultCallManager<_>>>>,
    > = DefaultExecutor::new(machine);

    // Apply all messages in the vector.
    for (i, m) in v.apply_messages.iter().enumerate() {
        let msg = Message::unmarshal_cbor(&m.bytes).unwrap();

        // Execute the message.
        let ret = exec.execute_message(msg, ApplyKind::Explicit).unwrap();

        // Compare the actual receipt with the expected receipt.
        let expected_receipt = &v.postconditions.receipts[i];
        if let Err(err) = check_msg_result(expected_receipt, &ret, i) {
            return VariantResult::Failed { id, reason: err };
        }
    }

    // Flush the machine, obtain the blockstore, and compare the
    // resulting state root with the expected state root.
    let final_root = match exec.flush() {
        Ok(cid) => cid,
        Err(err) => {
            return VariantResult::Failed {
                id,
                reason: err.context("flushing executor failed"),
            };
        }
    };

    bs = exec.consume().unwrap().consume().consume();

    if let Err(err) = compare_state_roots(&bs, &final_root, &v.postconditions.state_tree.root_cid) {
        return VariantResult::Failed {
            id,
            reason: err.context("comparing state roots failed"),
        };
    }

    VariantResult::Ok { id }
}
