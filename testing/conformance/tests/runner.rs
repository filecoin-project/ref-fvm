// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT
use std::collections::HashMap;
use std::env::var;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context as _, anyhow};
use colored::*;
use futures::{StreamExt, stream};
use fvm::engine::MultiEngine;
use fvm_conformance_tests::driver::*;
use fvm_conformance_tests::report;
use fvm_conformance_tests::tracing::{TestTraceExporter, TestTraceExporterRef};
use fvm_conformance_tests::vector::{MessageVector, Selector};
use fvm_conformance_tests::vm::{TestStatsGlobal, TestStatsRef};
use itertools::Itertools;
use lazy_static::lazy_static;
use walkdir::WalkDir;

enum ErrorAction {
    Error,
    Warn,
    Ignore,
}

impl FromStr for ErrorAction {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "error" => Ok(Self::Error),
            "warn" => Ok(Self::Warn),
            "ignore" => Ok(Self::Ignore),
            _ => Err("must be one of error|warn|ignore".into()),
        }
    }
}

lazy_static! {
    /// The maximum parallelism when processing test vectors. Capped at 48.
    static ref TEST_VECTOR_PARALLELISM: usize = std::env::var_os("TEST_VECTOR_PARALLELISM")
        .map(|s| {
            let s = s.to_str().unwrap();
            s.parse().expect("parallelism must be an integer")
        })
        .unwrap_or_else(num_cpus::get)
        .min(48);

    /// By default a post-condition error is fatal and stops all testing. We can use this env var to relax that
    /// and let the test carry on (optionally with a warning); there's a correctness check against the post condition anyway.
    static ref TEST_VECTOR_POSTCONDITION_MISSING_ACTION: ErrorAction = std::env::var_os("TEST_VECTOR_POSTCONDITION_MISSING_ACTION")
        .map(|s| {
            let s = s.to_str().unwrap();
            s.parse().expect("unexpected post condition error action")
        })
        .unwrap_or(ErrorAction::Warn);

    static ref ENGINES: MultiEngine = MultiEngine::new(*TEST_VECTOR_PARALLELISM as u32);
}

#[tokio::test(flavor = "multi_thread")]
async fn conformance_test_runner() -> anyhow::Result<()> {
    env_logger::init();

    let path = var("VECTOR").unwrap_or_else(|_| "test-vectors/corpus".to_owned());
    let path = Path::new(path.as_str()).to_path_buf();
    let stats = TestStatsGlobal::new_ref();

    // Optionally create a component to export gas charge traces.
    let tracer = std::env::var("TRACE_DIR")
        .ok()
        .map(|path| TestTraceExporter::new(Path::new(path.as_str()).to_path_buf()));

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

    let stats = Arc::new(stats);
    let tracer = Arc::new(tracer);

    // Process all test vectors concurrently, limited by buffer_unordered
    let results = stream::iter(vector_paths)
        .map(|path| {
            let stats = stats.clone();
            let tracer = tracer.clone();

            async move {
                // Run the vector processing in a blocking task
                tokio::task::spawn_blocking(move || run_vector(path, stats, tracer)).await?
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

    if let Some(stats) = stats.as_ref().as_ref() {
        let stats = stats.lock().unwrap();
        println!(
            "{}",
            format!(
                "memory stats:\n init.min: {}\n init.max: {}\n exec.min: {}\n exec.max: {}\n",
                stats.init.min_instance_memory_bytes,
                stats.init.max_instance_memory_bytes,
                stats.exec.min_instance_memory_bytes,
                stats.exec.max_instance_memory_bytes,
            )
            .bold()
        );
    }

    if let Some(tracer) = tracer.as_ref() {
        tracer.export_tombstones()?;
    }

    if failed > 0 {
        Err(anyhow!("some vectors failed"))
    } else {
        Ok(())
    }
}

/// Runs a single test vector and returns a list of VectorResults,
/// one per variant.
fn run_vector(
    path: PathBuf,
    stats: Arc<TestStatsRef>,
    tracer: Arc<TestTraceExporterRef>,
) -> anyhow::Result<(PathBuf, Vec<VariantResult>)> {
    let file = File::open(&path)?;
    let reader = BufReader::new(file);

    // Parse the test vector
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
                let msg = format!(
                    "imported roots ({}) do not contain postcondition CID {}",
                    imported_root.iter().join(", "),
                    v.postconditions.state_tree.root_cid
                );

                match *TEST_VECTOR_POSTCONDITION_MISSING_ACTION {
                    ErrorAction::Error => {
                        anyhow::bail!(msg);
                    }
                    ErrorAction::Warn => {
                        eprintln!("WARN: {msg} in {}", path.display())
                    }
                    ErrorAction::Ignore => (),
                }
            }

            // Run all variants
            let results = v
                .preconditions
                .variants
                .iter()
                .map(|variant| {
                    let variant_id = variant.id.clone();
                    let name = format!("{} | {}", path.display(), variant_id);

                    run_variant(
                        bs.clone(),
                        &v,
                        variant,
                        &ENGINES,
                        true,
                        stats.as_ref().clone(),
                        tracer
                            .as_ref()
                            .clone()
                            .map(|t| t.export_fun(path.clone(), variant_id.clone())),
                    )
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
