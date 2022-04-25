use std::time::{Duration, Instant};

use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use fvm::call_manager::ExecutionStats;
use fvm::gas::tracer::{Consumption, Context, Event, GasTrace, Point};
use rand::{thread_rng, Rng};

pub fn benchmark_gas_tracing(c: &mut Criterion) {
    let mut group = c.benchmark_group("gas_tracing");

    for size in [16, 32, 64, 128, 256].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        let cid = cid::Cid::default();
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched_ref(
                || GasTrace::start(),
                move |gt| {
                    let ctx = Context {
                        code_cid: cid.clone(),
                        method_num: size,
                    };
                    let point = Point {
                        event: Event::Started,
                        label: "foo".to_string(),
                    };
                    let consumption = Consumption {
                        fuel_consumed: Some(1234 + size),
                        gas_consumed: Some((1111 + size) as i64),
                    };
                    gt.record(ctx, point, consumption);
                },
                BatchSize::SmallInput,
            );
        });
    }
}

pub fn benchmark_accumulator(c: &mut Criterion) {
    let mut group = c.benchmark_group("accumulator");

    group.bench_function("exec stats accumulator", |b| {
        // generate a random number to perform math with
        let r: u64 = thread_rng().gen();
        b.iter_batched_ref(
            || (ExecutionStats::default(), Instant::now()),
            move |(exec_stats, now)| {
                let call_duration = now.elapsed();
                exec_stats.fuel_used += r;
                exec_stats.call_count += 1;
                exec_stats.call_overhead += call_duration;
                exec_stats.wasm_duration +=
                    (call_duration + call_duration).max(Duration::default());
            },
            BatchSize::SmallInput,
        )
    });
}

pub fn benchmark_time(c: &mut Criterion) {
    let mut group = c.benchmark_group("time");

    group.bench_function("std::time::Instant::now()", |b| {
        b.iter(|| black_box(std::time::Instant::now()))
    });
    group.bench_function("minstant::Instant::now()", |b| {
        b.iter(|| black_box(minstant::Instant::now()))
    });
}

criterion_group!(
    benches,
    benchmark_gas_tracing,
    benchmark_accumulator,
    benchmark_time
);

criterion_main!(benches);
