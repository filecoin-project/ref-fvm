# Test vector runner and benchmarker

This directory contains tooling to run test vectors against the FVM in the form
of tests and benchmarks.

## Benchmark notes

**Build**

```shell
cargo build --release --bin perf-conformance
```

**Smoke test**

For a single vector:

```shell
CARGO_PROFILE_BENCH_DEBUG=true \
  VECTOR=testing/conformance/test-vectors/corpus/specs_actors_v6/TestMeasurePreCommitGas/ff3438ebc9c42d99d23a8654c4a5d5c8408f575950c05e504be9de58aa521167-t0100-t0101-storageminer-25.json \
  ./target/release/perf-conformance
```

**Run benchmark**

For a single vector:

```shell
CARGO_PROFILE_BENCH_DEBUG=true \
  VECTOR=testing/conformance/test-vectors/corpus/specs_actors_v6/TestMeasurePreCommitGas/ff3438ebc9c42d99d23a8654c4a5d5c8408f575950c05e504be9de58aa521167-t0100-t0101-storageminer-25.json \
  perf record -k mono ./target/release/perf-conformance
```

**Add the JIT data**

> Note: Dumps random files everywhere.

```shell
perf inject --jit --input perf.data --output perf.jit.data
```

**Generate a report**

```shell
perf report --input perf.jit.data --hierarchy
```