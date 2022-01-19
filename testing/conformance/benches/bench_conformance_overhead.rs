extern crate criterion;
use std::env::var;
use std::path::{Path, PathBuf};
use std::time::Duration;

use conformance_tests::test_utils::*;
use criterion::*;
use walkdir::WalkDir;

mod bench_utils;
use crate::bench_utils::bench_vector_file;

fn bench_no_messages(
    group: &mut BenchmarkGroup<measurement::WallTime>,
    path_to_setup: PathBuf,
) -> anyhow::Result<()> {
    // compute measurement overhead by benching running a single empty vector of zero messages
    match &bench_vector_file(
        group,
        path_to_setup,
        Some(vec![]),
        true,
        Some("bench_no_messages".parse().unwrap()),
        true,
    )?[0]
    {
        VariantResult::Ok { .. } => Ok(()),
        VariantResult::Skipped { reason, id } => Err(anyhow::anyhow!(
            "no messages test {} skipped due to {}",
            id,
            reason
        )),
        VariantResult::Failed { reason, id } => Err(anyhow::anyhow!(
            "no messages test {} failed due to {}",
            id,
            reason
        )),
    }
}

fn bench_noops(
    _group: &mut BenchmarkGroup<measurement::WallTime>,
    _path_to_setup: PathBuf,
) -> anyhow::Result<()> {
    // TODO compute a different measurement overhead by benching a vector file that just sends a ton of really low-effort messages
    Err(anyhow::anyhow!("unimplemented"))
}

fn bench_conformance_overhead(c: &mut Criterion) {
    pretty_env_logger::init();

    let path_to_setup = match var("VECTOR") {
        Ok(v) => Path::new(v.as_str()).to_path_buf(),
        Err(_) => WalkDir::new("test-vectors/corpus")
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(is_runnable)
            .map(|e| e.path().to_path_buf())
            .next()
            .unwrap(),
    };

    // TODO: this is 30 seconds per benchmark... yeesh! once we get the setup running faster (by cloning VMs more efficiently), we can probably bring this down.
    let mut group = c.benchmark_group("measurement-overhead-baselines");
    group.measurement_time(Duration::new(30, 0));
    // start by getting some baselines!
    // TODO real error handling
    bench_no_messages(&mut group, path_to_setup.clone()).unwrap();
    //bench_noops(&mut group, path_to_setup).unwrap();
    group.finish();
    // TODO FIX WHY THIS ISN"T RUNNING UUUUGH
}

criterion_group!(benches_overhead, bench_conformance_overhead);
criterion_main!(benches_overhead);
