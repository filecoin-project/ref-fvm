use std::time::Duration;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use fvm::gas::tracer::{Consumption, Context, Event, GasTrace, Point};

pub fn benchmark_gas_tracing(c: &mut Criterion) {
    let mut group = c.benchmark_group("gas_tracing");

    for size in [16, 32, 64, 128, 256].iter() {
        group.warm_up_time(Duration::from_secs(5));
        group.measurement_time(Duration::from_secs(20));
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter_batched_ref(
                || GasTrace::start(),
                move |gt| {
                    let ctx = Context {
                        code_cid: Default::default(),
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

criterion_group!(benches, benchmark_gas_tracing);
criterion_main!(benches);
