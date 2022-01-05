// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::{anyhow, Result};
use async_std::stream;
use cid::Cid;
use conformance_tests::vector::{MessageVector, Selector, TestVector, Variant};
use conformance_tests::vm::{TestCallManager, TestKernel, TestMachine};
use fmt::Display;
use futures::{StreamExt, TryStreamExt};
use fvm::call_manager::DefaultCallManager;
use fvm::executor::{ApplyKind, ApplyRet, DefaultExecutor, Executor};
use fvm::machine::Machine;
use fvm::DefaultKernel;
use fvm_shared::blockstore::{self, MemoryBlockstore};
use fvm_shared::encoding::Cbor;
use fvm_shared::message::Message;
use fvm_shared::receipt::Receipt;
use lazy_static::lazy_static;
use regex::Regex;
use std::env::var;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::{fmt, iter};
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
async fn conformance_test_runner() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let vector_results = match var("VECTOR") {
        Ok(v) => either::Either::Left(
            iter::once(async move {
                let path = Path::new(v.as_str()).to_path_buf();
                let res = run_vector(&path).await?;
                anyhow::Ok((path, res))
            })
            .map(futures::future::Either::Left),
        ),
        Err(_) => either::Either::Right(
            WalkDir::new("test-vectors/corpus")
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(is_runnable)
                .map(|e| async move {
                    let path = e.path().to_path_buf();
                    let res = run_vector(&path).await?;
                    Ok((path, res))
                })
                .map(futures::future::Either::Right),
        ),
    };

    let mut results = stream::from_iter(vector_results)
        // Will _load_ up to 100 vectors at once in any order. We won't actually run the vectors in
        // parallel (yet), but that shouldn't be too hard.
        .buffer_unordered(100)
        .map_ok(|(path, res)| stream::from_iter(res).map_ok(move |r| (path.clone(), r)))
        .try_flatten();

    let mut succeeded = 0;
    let mut failed = 0;
    let mut skipped = 0;

    while let Some((path, res)) = results.next().await.transpose()? {
        match res {
            VariantResult::Ok { id } => {
                println!("OK vector {}, variant {}", path.display(), id);
                succeeded += 1;
            }
            VariantResult::Failed { reason, id } => {
                println!(
                    "FAIL vector {}, variant {}, reason: {:?}",
                    path.display(),
                    id,
                    reason
                );
                failed += 1;
            }
            VariantResult::Skipped { reason, id } => {
                println!(
                    "SKIP vector {}, variant {}, reason: {:?}",
                    path.display(),
                    id,
                    reason
                );
                skipped += 1;
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
        Err(anyhow!("some vectors failed"))
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
async fn run_vector(
    path: &PathBuf,
) -> anyhow::Result<impl Iterator<Item = anyhow::Result<VariantResult>>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let vector: TestVector = serde_json::from_reader(reader)?;

    match vector {
        TestVector::Message(v) => {
            let skip = !v.selector.as_ref().map_or(true, Selector::supported);
            if skip {
                Ok(either::Either::Left(
                    v.preconditions.variants.into_iter().map(|variant| {
                        Ok(VariantResult::Skipped {
                            id: variant.id,
                            reason: "selector not supported".to_owned(),
                        })
                    }),
                ))
            } else {
                // First import the blockstore and do some sanity checks.
                let (bs, imported_root) = v.seed_blockstore().await?;
                if imported_root.len() != 1 {
                    return Err(anyhow!("expected one root; found {}", imported_root.len()));
                }
                if v.preconditions.state_tree.root_cid != imported_root[0] {
                    return Err(anyhow!(
                        "imported root does not match precondition root; imported: {}; expected: {}",
                        imported_root[0],
                        v.preconditions.state_tree.root_cid
                    ));
                }
                Ok(either::Either::Right(
                    (0..v.preconditions.variants.len())
                        .map(move |i| run_variant(bs.clone(), &v, &v.preconditions.variants[i])),
                ))
            }
        }
    }
}

fn run_variant(
    bs: MemoryBlockstore,
    v: &MessageVector,
    variant: &Variant,
) -> anyhow::Result<VariantResult> {
    let id = variant.id.clone();

    // Construct the Machine.
    let machine = TestMachine::new_for_vector(&v, &variant, bs);
    let mut exec: DefaultExecutor<
        TestKernel<DefaultKernel<TestCallManager<DefaultCallManager<_>>>>,
    > = DefaultExecutor::new(machine);

    // Apply all messages in the vector.
    for (i, m) in v.apply_messages.iter().enumerate() {
        let msg = Message::unmarshal_cbor(&m.bytes)?;

        // Execute the message.
        let ret = match exec.execute_message(msg, ApplyKind::Explicit) {
            Ok(ret) => ret,
            Err(e) => return Ok(VariantResult::Failed { id, reason: e }),
        };

        // Compare the actual receipt with the expected receipt.
        let expected_receipt = &v.postconditions.receipts[i];
        if let Err(err) = check_msg_result(expected_receipt, &ret, i) {
            return Ok(VariantResult::Failed { id, reason: err });
        }
    }

    // Flush the machine, obtain the blockstore, and compare the
    // resulting state root with the expected state root.
    let final_root = match exec.flush() {
        Ok(cid) => cid,
        Err(err) => {
            return Ok(VariantResult::Failed {
                id,
                reason: err.context("flushing executor failed"),
            });
        }
    };

    let machine = match exec.consume() {
        Some(machine) => machine,
        None => {
            return Ok(VariantResult::Failed {
                id,
                reason: anyhow!("machine poisoned"),
            })
        }
    };

    let bs = machine.consume().consume();

    if let Err(err) = compare_state_roots(&bs, &final_root, &v.postconditions.state_tree.root_cid) {
        return Ok(VariantResult::Failed {
            id,
            reason: err.context("comparing state roots failed"),
        });
    }

    Ok(VariantResult::Ok { id })
}
