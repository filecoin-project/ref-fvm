// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT
//#[macro_use]
extern crate criterion;
// TODO document how to use this?
use std::env::var;
use std::iter;
use std::path::{Path, PathBuf};
use std::time::Duration;

use colored::Colorize;
use conformance_tests::test_utils::*;
use criterion::*;
use walkdir::WalkDir;

mod bench_utils;

use crate::bench_utils::bench_vector_file;

// TODO might be nice to add a command line option to not run test first?
fn bench_conformance(c: &mut Criterion) {
    pretty_env_logger::init();

    // TODO match globs to get whole folders?
    let (mut vector_results, _is_many): (Vec<PathBuf>, bool) = match var("VECTOR") {
        Ok(v) => (
            iter::once(Path::new(v.as_str()).to_path_buf()).collect(),
            false,
        ),
        Err(_) => (
            WalkDir::new("test-vectors/corpus")
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(is_runnable)
                .map(|e| e.path().to_path_buf())
                .collect(),
            true,
        ),
    };

    // TODO: this is 30 seconds per benchmark... yeesh! once we get the setup running faster (by cloning VMs more efficiently), we can probably bring this down.
    let mut group = c.benchmark_group("conformance-tests");
    group.measurement_time(Duration::new(30, 0));

    let mut succeeded = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut corrupt_files = 0;

    // Output the result to stdout.
    // Doing this here instead of in an inspect so that we get streaming output.
    macro_rules! report {
        ($status:expr, $path:expr, $id:expr) => {
            println!("[{}] vector: {} | variant: {}", $status, $path, $id);
        };
    }

    for vector in vector_results.drain(..) {
        match bench_vector_file(&mut group, vector.clone(), None, false, None, false) {
            Ok(vrs) => {
                vrs.iter()
                    .map(|vr| match vr {
                        VariantResult::Ok { id } => {
                            report!("OKAY/BENCHED".on_green(), vector.display(), id);
                            succeeded += 1;
                        }
                        VariantResult::Failed { reason, id } => {
                            report!("FAIL/NOT BENCHED".white().on_red(), vector.display(), id);
                            println!("\t|> reason: {:#}", reason);
                            failed += 1;
                        }
                        VariantResult::Skipped { reason, id } => {
                            report!("SKIP/NOT BENCHED".on_yellow(), vector.display(), id);
                            println!("\t|> reason: {:#}", reason);
                            skipped += 1;
                        }
                    })
                    .for_each(drop);
            }
            Err(e) => {
                report!(
                    "FILE FAIL/NOT BENCHED".white().on_purple(),
                    vector.display(),
                    "n/a"
                );
                println!("\t|> reason: {:#}", e.to_string());
                corrupt_files += 1;
            }
        }
    }

    println!();
    println!(
        "{}",
        format!(
            "benchmarking tests result: {}/{} tests benchmarked ({} skipped, {} failed, {} vector files unparseable)",
            succeeded,
            failed + succeeded + skipped + corrupt_files,
            skipped,
            failed,
            corrupt_files
        )
            .bold()
    );

    group.finish();
}

criterion_group!(benches, bench_conformance);
criterion_main!(benches);
