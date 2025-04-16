// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::env::var;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use colored::*;
use fvm::engine::MultiEngine;
use fvm_conformance_tests::driver::*;
use fvm_conformance_tests::report;
use fvm_conformance_tests::tracing::TestTraceExporter;
use fvm_conformance_tests::vector::MessageVector;
use fvm_conformance_tests::vm::TestStatsGlobal;
use fvm_ipld_blockstore::MemoryBlockstore;
use itertools::Itertools;
use walkdir::WalkDir;

#[test]
fn conformance_test_runner() -> anyhow::Result<()> {
    env_logger::init();

    let parallelism = rayon::current_num_threads().try_into().unwrap();
    let engine: MultiEngine = MultiEngine::new(parallelism);
    println!("running with {parallelism} threads");

    let path = var("VECTOR").unwrap_or_else(|_| "test-vectors/corpus".to_owned());
    let path = Path::new(path.as_str()).to_path_buf();
    let stats = TestStatsGlobal::new_ref();

    // Optionally create a component to export gas charge traces.
    let tracer = std::env::var("TRACE_DIR")
        .ok()
        .map(|path| TestTraceExporter::new(Path::new(path.as_str()).to_path_buf()));

    // Collect the test vector files.
    let vector_paths = if path.is_file() {
        vec![path]
    } else {
        WalkDir::new(path)
            .into_iter()
            .filter_ok(is_runnable)
            .map_ok(|de| de.into_path())
            .collect::<Result<Vec<_>, _>>()?
    };

    // Collect the test vectors.
    let vectors: Vec<(PathBuf, MessageVector)> = vector_paths
        .into_par_iter()
        .map(|p| {
            let v = MessageVector::from_file(&p)?;
            Ok((p, v))
        })
        .collect::<anyhow::Result<_>>()?;

    #[derive(Default)]
    struct Counters {
        succeeded: u32,
        failed: u32,
        skipped: u32,
    }

    let counters: Counters = vectors
        .par_iter()
        .flat_map_iter({
            let stats = &stats;
            let tracer = &tracer;
            let engine = &engine;
            move |(path, vector)| {
                use rayon::iter::Either;

                // Skip unsupported vectors.
                if !vector.is_supported() {
                    return Either::Left(vector.preconditions.variants.iter().map(
                        move |variant| {
                            (
                                path,
                                VariantResult::Skipped {
                                    reason: "unsupported".into(),
                                    id: variant.id.clone(),
                                },
                            )
                        },
                    ));
                }

                // Load the vector's blockstore.
                let bs = load_vector_bs(vector);

                Either::Right(vector.preconditions.variants.iter().map(move |variant| {
                    let bs = match &bs {
                        Ok(bs) => bs.clone(),
                        Err(e) => {
                            return (
                                path,
                                VariantResult::Failed {
                                    reason: anyhow!("failed to load vector state: {e}"),
                                    id: variant.id.clone(),
                                },
                            );
                        }
                    };
                    let res = run_variant(
                        bs.clone(),
                        vector,
                        variant,
                        engine,
                        true,
                        stats.clone(),
                        tracer
                            .clone()
                            .map(|t| t.export_fun(path.clone(), variant.id.clone())),
                    )
                    .unwrap_or_else(|e| VariantResult::Failed {
                        reason: e,
                        id: variant.id.clone(),
                    });
                    (path, res)
                }))
            }
        })
        .map(|(path, res)| match res {
            VariantResult::Ok { id } => {
                report!("OK".on_green(), path.display(), id);
                Counters {
                    succeeded: 1,
                    ..Default::default()
                }
            }
            VariantResult::Failed { reason, id } => {
                report!("FAIL".white().on_red(), path.display(), id, reason);
                Counters {
                    failed: 1,
                    ..Default::default()
                }
            }
            VariantResult::Skipped { reason, id } => {
                report!("SKIP".on_yellow(), path.display(), id, reason);
                Counters {
                    skipped: 1,
                    ..Default::default()
                }
            }
        })
        .reduce(Counters::default, |a, b| Counters {
            succeeded: a.succeeded + b.succeeded,
            failed: a.failed + b.failed,
            skipped: a.skipped + b.skipped,
        });

    println!();
    println!(
        "{}",
        format!(
            "conformance tests result: {}/{} tests passed ({} skipped)",
            counters.succeeded,
            counters.failed + counters.succeeded,
            counters.skipped,
        )
        .bold()
    );

    if let Some(ref stats) = stats {
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

    if let Some(ref tracer) = tracer {
        tracer.export_tombstones()?;
    }

    if counters.failed > 0 {
        Err(anyhow!("some vectors failed"))
    } else {
        Ok(())
    }
}

fn load_vector_bs(v: &MessageVector) -> anyhow::Result<MemoryBlockstore> {
    let (bs, imported_root) = v.seed_blockstore()?;
    if !imported_root.contains(&v.preconditions.state_tree.root_cid) {
        return Err(anyhow!(
            "imported roots ({}) do not contain precondition CID {}",
            imported_root.iter().join(", "),
            v.preconditions.state_tree.root_cid
        ));
    }
    if !imported_root.contains(&v.postconditions.state_tree.root_cid) {
        let msg = format!(
            "imported roots ({}) do not contain postcondition CID {}",
            imported_root.iter().join(", "),
            v.postconditions.state_tree.root_cid
        );

        return Err(anyhow!(msg));
    }
    Ok(bs)
}
