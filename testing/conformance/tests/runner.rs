// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::collections::HashMap;
use std::env::var;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::{fmt, iter};

use anyhow::{anyhow, Context as _, Result};
use async_std::{stream, sync, task};
use cid::Cid;
use colored::*;
use conformance_tests::vector::{MessageVector, Selector, Variant};
use conformance_tests::vm::{TestKernel, TestMachine};
use fmt::Display;
use futures::{Future, StreamExt, TryFutureExt, TryStreamExt};
use fvm::executor::{ApplyKind, ApplyRet, DefaultExecutor, Executor};
use fvm::kernel::Context;
use fvm::machine::Machine;
use fvm::state_tree::{ActorState, StateTree};
use fvm_shared::address::Protocol;
use fvm_shared::blockstore::{CborStore, MemoryBlockstore};
use fvm_shared::crypto::signature::SECP_SIG_LEN;
use fvm_shared::encoding::Cbor;
use fvm_shared::message::Message;
use fvm_shared::receipt::Receipt;
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use walkdir::{DirEntry, WalkDir};

lazy_static! {
    static ref SKIP_TESTS: Vec<Regex> = vec![
        // currently empty.
    ];
    /// The maximum parallelism when processing test vectors.
    static ref TEST_VECTOR_PARALLELISM: usize = std::env::var_os("TEST_VECTOR_PARALLELISM")
        .map(|s| {
            let s = s.to_str().unwrap();
            s.parse().expect("parallelism must be an integer")
        }).unwrap_or_else(num_cpus::get);
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
        .failure_info
        .as_ref()
        .map(|e| e.to_string())
        .unwrap_or_else(|| "no error".into());
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

fn compare_actors(
    bs: &MemoryBlockstore,
    identifier: impl Display,
    actual: Option<ActorState>,
    expected: Option<ActorState>,
) -> Result<()> {
    if actual == expected {
        return Ok(());
    }
    log::error!(
        "{} actor state differs: {:?} != {:?}",
        identifier,
        actual,
        expected
    );

    match (actual, expected) {
        (Some(a), Some(e)) if a.state != e.state => {
            let a_root: Vec<serde_cbor::Value> = bs.get_cbor(&a.state)?.unwrap();
            let e_root: Vec<serde_cbor::Value> = bs.get_cbor(&e.state)?.unwrap();
            if a_root.len() != e_root.len() {
                log::error!("states have different numbers of fields")
            } else {
                for (f, (af, ef)) in a_root.iter().zip(e_root.iter()).enumerate() {
                    if af != ef {
                        log::error!("mismatched field {}: {:#?} != {:#?}", f, af, ef);
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Compares the state-root with the postcondition state-root in the test vector. If they don't
/// match, it performs a basic actor & state-diff of the message senders and receivers in the test
/// vector, along with all system actors.
fn compare_state_roots(bs: &MemoryBlockstore, root: &Cid, vector: &MessageVector) -> Result<()> {
    if root == &vector.postconditions.state_tree.root_cid {
        return Ok(());
    }

    let actual_st =
        StateTree::new_from_root(bs, root).context("failed to load actual state tree")?;
    let expected_st = StateTree::new_from_root(bs, &vector.postconditions.state_tree.root_cid)
        .context("failed to load expected state tree")?;

    // We only compare system actors and the send/receiver actor as we don't know what other actors
    // might exist in the state-tree (it's usually incomplete).

    for m in &vector.apply_messages {
        let msg = Message::unmarshal_cbor(&m.bytes)?;
        let actual_actor = actual_st.get_actor(&msg.from)?;
        let expected_actor = expected_st.get_actor(&msg.from)?;
        compare_actors(bs, "sender", actual_actor, expected_actor)?;

        let actual_actor = actual_st.get_actor(&msg.to)?;
        let expected_actor = expected_st.get_actor(&msg.to)?;
        compare_actors(bs, "receiver", actual_actor, expected_actor)?;
    }

    // All system actors
    for id in 0..100 {
        let expected_actor = match expected_st.get_actor_id(id) {
            Ok(act) => act,
            Err(_) => continue, // we don't expect it anyways.
        };
        let actual_actor = actual_st.get_actor_id(id)?;
        compare_actors(
            bs,
            format_args!("builtin {}", id),
            actual_actor,
            expected_actor,
        )?;
    }

    return Err(anyhow!(
        "wrong post root cid; expected {}, but got {}",
        &vector.postconditions.state_tree.root_cid,
        root
    ));
}

#[async_std::test]
async fn conformance_test_runner() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let vector_results = match var("VECTOR") {
        Ok(v) => either::Either::Left(
            iter::once(async move {
                let path = Path::new(v.as_str()).to_path_buf();
                let res = run_vector(path.clone()).await?;
                anyhow::Ok((path, res))
            })
            .map(futures::future::Either::Left),
        ),
        Err(_) => either::Either::Right(
            WalkDir::new("test-vectors/corpus")
                .into_iter()
                .filter_ok(is_runnable)
                .map(|e| async move {
                    let path = e?.path().to_path_buf();
                    let res = run_vector(path.clone()).await?;
                    Ok((path, res))
                })
                .map(futures::future::Either::Right),
        ),
    };

    let mut results = Box::pin(
        stream::from_iter(vector_results)
            // Will _load_ up to 100 vectors at once in any order. We won't actually run the vectors in
            // parallel (yet), but that shouldn't be too hard.
            .map(|task| {
                async move {
                    let (path, jobs) = task.await?;
                    Ok(stream::from_iter(jobs).map(move |job| {
                        let path = path.clone();
                        Ok(async move { anyhow::Ok((path, job.await?)) })
                    }))
                }
                .try_flatten_stream()
            })
            .flatten()
            .try_buffer_unordered(*TEST_VECTOR_PARALLELISM),
    );

    let mut succeeded = 0;
    let mut failed = 0;
    let mut skipped = 0;

    // Output the result to stdout.
    // Doing this here instead of in an inspect so that we get streaming output.
    macro_rules! report {
        ($status:expr, $path:expr, $id:expr) => {
            println!("[{}] vector: {} | variant: {}", $status, $path, $id);
        };
    }

    while let Some((path, res)) = results.next().await.transpose()? {
        match res {
            VariantResult::Ok { id } => {
                report!("OK".on_green(), path.display(), id);
                succeeded += 1;
            }
            VariantResult::Failed { reason, id } => {
                report!("FAIL".white().on_red(), path.display(), id);
                println!("\t|> reason: {:#}", reason);
                failed += 1;
            }
            VariantResult::Skipped { reason, id } => {
                report!("SKIP".on_yellow(), path.display(), id);
                println!("\t|> reason: {}", reason);
                skipped += 1;
            }
        }
    }

    println!();
    println!(
        "{}",
        format!(
            "conformance tests result: {}/{} tests passed ({} skipped)",
            succeeded,
            failed + succeeded,
            skipped,
        )
        .bold()
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
    path: PathBuf,
) -> anyhow::Result<impl Iterator<Item = impl Future<Output = anyhow::Result<VariantResult>>>> {
    let file = File::open(&path)?;
    let reader = BufReader::new(file);

    // Test vectors have the form:
    //
    //     { "class": ..., rest... }
    //
    // Unfortunately:
    // 1. That means we need to use serde's "flatten" and/or "tag" feature to decode them.
    // 2. Serde's JSON library doesn't support arbitrary precision numbers when doing this.
    // 3. The circulating supply doesn't fit in a u64, and f64 isn't precise enough.
    //
    // So we manually:
    // 1. Decode into a map of `String` -> `raw data`.
    // 2. Pull off the class.
    // 3. Re-serialize.
    // 4. Decode into the correct type.
    //
    // Upstream bug is https://github.com/serde-rs/serde/issues/1183 (or at least that looks like
    // the most appropriate one out of all the related issues).
    let mut vector: HashMap<String, Box<serde_json::value::RawValue>> =
        serde_json::from_reader(reader)?;
    let class_json = vector
        .remove("class")
        .context("expected test vector to have a class")?;

    let class: &str = serde_json::from_str(class_json.get())?;
    let vector_json = serde_json::to_string(&vector)?;

    match class {
        "message" => {
            let v: MessageVector = serde_json::from_str(&vector_json)?;
            let skip = !v.selector.as_ref().map_or(true, Selector::supported);
            if skip {
                Ok(either::Either::Left(
                    v.preconditions.variants.into_iter().map(|variant| {
                        futures::future::Either::Left(async move {
                            Ok(VariantResult::Skipped {
                                id: variant.id,
                                reason: "selector not supported".to_owned(),
                            })
                        })
                    }),
                ))
            } else {
                // First import the blockstore and do some sanity checks.
                let (bs, imported_root) = v.seed_blockstore().await?;
                if !imported_root.contains(&v.preconditions.state_tree.root_cid) {
                    return Err(anyhow!(
                        "imported roots ({}) do not contain precondition CID {}",
                        imported_root.iter().join(", "),
                        v.preconditions.state_tree.root_cid
                    ));
                }
                if !imported_root.contains(&v.postconditions.state_tree.root_cid) {
                    return Err(anyhow!(
                        "imported roots ({}) do not contain postcondition CID {}",
                        imported_root.iter().join(", "),
                        v.preconditions.state_tree.root_cid
                    ));
                }

                let v = sync::Arc::new(v);
                Ok(either::Either::Right(
                    (0..v.preconditions.variants.len()).map(move |i| {
                        let v = v.clone();
                        let bs = bs.clone();
                        let name =
                            format!("{} | {}", path.display(), &v.preconditions.variants[i].id);
                        futures::future::Either::Right(
                                task::Builder::new()
                                    .name(name)
                                    .spawn(async move {
                                        run_variant(bs, &v, &v.preconditions.variants[i])
                                    }).unwrap(),
                            )
                    }),
                ))
            }
        }
        other => return Err(anyhow!("unknown test vector class: {}", other)),
    }
}

fn run_variant(
    bs: MemoryBlockstore,
    v: &MessageVector,
    variant: &Variant,
) -> anyhow::Result<VariantResult> {
    let id = variant.id.clone();

    // Construct the Machine.
    let machine = TestMachine::new_for_vector(v, variant, bs);
    let mut exec: DefaultExecutor<TestKernel> = DefaultExecutor::new(machine);

    // Apply all messages in the vector.
    for (i, m) in v.apply_messages.iter().enumerate() {
        let msg = Message::unmarshal_cbor(&m.bytes)?;

        // Execute the message.
        let mut raw_length = m.bytes.len();
        if msg.from.protocol() == Protocol::Secp256k1 {
            // 65 bytes signature + 1 byte type + 3 bytes for field info.
            raw_length += SECP_SIG_LEN + 4;
        }
        let ret = match exec.execute_message(msg, ApplyKind::Explicit, raw_length) {
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

    if let Err(err) = compare_state_roots(&bs, &final_root, v) {
        return Ok(VariantResult::Failed {
            id,
            reason: err.context("comparing state roots failed"),
        });
    }

    Ok(VariantResult::Ok { id })
}
