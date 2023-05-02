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
  VECTOR=testing/conformance/test-vectors/corpus/extracted/0001-shark-01/fil_9_storageminer/PreCommitSector/Ok/ext-0001-fil_9_storageminer-PreCommitSector-Ok-1.json \
  ./target/release/perf-conformance
```

**Run benchmark**

For a single vector:

```shell
CARGO_PROFILE_BENCH_DEBUG=true \
  VECTOR=testing/conformance/test-vectors/corpus/extracted/0001-shark-01/fil_9_storageminer/PreCommitSector/Ok/ext-0001-fil_9_storageminer-PreCommitSector-Ok-1.json \
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

## Adding new actor bundles

To add support for new actors releases, take the bundle [tar file from lotus](https://github.com/filecoin-project/lotus/tree/master/build/actors), add it to `testing/conformance/actors/`, and register it in `testing/conformance/src/actors.rs`.
