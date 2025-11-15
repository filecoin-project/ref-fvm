// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::collections::HashMap;
use std::env::var;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, anyhow};
use colored::*;
use futures::{StreamExt, stream};
use fvm::machine::MultiEngine;
use fvm_conformance_tests::driver::*;
use fvm_conformance_tests::report;
use fvm_conformance_tests::vector::{MessageVector, Selector};
use itertools::Itertools;
use lazy_static::lazy_static;
use walkdir::WalkDir;

lazy_static! {
    /// The maximum parallelism when processing test vectors. Capped at 48.
    static ref TEST_VECTOR_PARALLELISM: usize = std::env::var_os("TEST_VECTOR_PARALLELISM")
        .map(|s| {
            let s = s.to_str().unwrap();
            s.parse().expect("parallelism must be an integer")
        })
        .unwrap_or_else(num_cpus::get)
        .min(48);

    static ref ENGINES: MultiEngine = MultiEngine::new();
}

#[tokio::test(flavor = "multi_thread")]
async fn conformance_test_runner() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let path = var("VECTOR").unwrap_or_else(|_| "test-vectors/corpus".to_owned());
    let path = Path::new(path.as_str()).to_path_buf();

    // Collect test vector files
    let vector_paths = if path.is_file() {
        vec![path]
    } else {
        WalkDir::new(path)
            .into_iter()
            .filter_ok(is_runnable)
            .map(|e| e.map(|de| de.path().to_path_buf()))
            .collect::<Result<Vec<_>, _>>()?
    };

    // Process all test vectors concurrently, limited by buffer_unordered
    let results = stream::iter(vector_paths)
        .map(|path| {
            async move {
                // Run the vector processing in a blocking task
                tokio::task::spawn_blocking(move || run_vector(path)).await?
            }
        })
        .buffer_unordered(*TEST_VECTOR_PARALLELISM)
        .collect::<Vec<_>>()
        .await;

    let mut succeeded = 0;
    let mut failed = 0;
    let mut skipped = 0;

    // Process results
    for result in results.into_iter() {
        match result {
            Ok((path, variant_results)) => {
                for res in variant_results {
                    match res {
                        VariantResult::Ok { id } => {
                            report!("OK".on_green(), path.display(), id);
                            succeeded += 1;
                        }
                        VariantResult::Failed { reason, id } => {
                            report!("FAIL".white().on_red(), path.display(), id, reason);
                            failed += 1;
                        }
                        VariantResult::Skipped { reason, id } => {
                            report!("SKIP".on_yellow(), path.display(), id, reason);
                            skipped += 1;
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Error processing vector: {:#}", e);
                failed += 1;
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

/// Runs a single test vector and returns a list of VectorResults,
/// one per variant.
fn run_vector(path: PathBuf) -> anyhow::Result<(PathBuf, Vec<VariantResult>)> {
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
        serde_json::from_reader(reader).context("failed to parse vector")?;
    let class_json = vector
        .remove("class")
        .context("expected test vector to have a class")?;

    let class: &str =
        serde_json::from_str(class_json.get()).context("failed to parse test vector class")?;
    let vector_json = serde_json::to_string(&vector)?;

    match class {
        "message" => {
            let v: MessageVector =
                serde_json::from_str(&vector_json).context("failed to parse message vector")?;
            let skip = !v.selector.as_ref().is_none_or(Selector::supported);

            if skip {
                let results = v
                    .preconditions
                    .variants
                    .into_iter()
                    .map(|variant| VariantResult::Skipped {
                        id: variant.id,
                        reason: "selector not supported".to_owned(),
                    })
                    .collect();
                return Ok((path, results));
            }

            // Import the blockstore and do sanity checks
            let (bs, imported_root) = v.seed_blockstore()?;
            anyhow::ensure!(
                imported_root.contains(&v.preconditions.state_tree.root_cid),
                "imported roots ({}) do not contain precondition CID {}",
                imported_root.iter().join(", "),
                v.preconditions.state_tree.root_cid
            );
            if !imported_root.contains(&v.postconditions.state_tree.root_cid) {
                return Err(anyhow!(
                    "imported roots ({}) do not contain postcondition CID {}",
                    imported_root.iter().join(", "),
                    v.postconditions.state_tree.root_cid
                ));
            }

            // Run all variants
            let results = v
                .preconditions
                .variants
                .iter()
                .map(|variant| {
                    let variant_id = variant.id.clone();
                    let name = format!("{} | {}", path.display(), variant_id);

                    run_variant(bs.clone(), &v, variant, &ENGINES, true)
                        .with_context(|| format!("failed to run {name}"))
                        .unwrap_or_else(|e| VariantResult::Failed {
                            id: variant_id,
                            reason: e,
                        })
                })
                .collect();

            Ok((path, results))
        }
        other => Err(anyhow!("unknown test vector class: {}", other)),
    }
}
