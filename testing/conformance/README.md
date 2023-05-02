# Test vector runner and benchmarker

This directory contains tooling to run test vectors against the FVM in the form of tests and benchmarks.

## Checkout `test-vectors`

The instructions below assume that the testing/conformance/test-vectors submodule is checked out. Run:

```shell
git submodule update --init
```

## Instructions

- To run all tests, just run `cargo test`.
- To run all test vectors under a specific directory, run eg. `VECTOR=test-vectors/corpus/extracted cargo test conformance -- --nocapture`
- To run a specific test vector, run `VECTOR=test-vectors/corpus/REST_OF_TEST_VECTOR.json cargo test -- conformance --nocapture`
- To bench a specific test vector, run `VECTOR=test-vectors/corpus/REST_OF_TEST_VECTOR.json cargo bench -- conformance --nocapture`
- To bench the system's overhead for the setup of the machine for a given test vector, run `VECTOR=test-vectors/corpus/REST_OF_TEST_VECTOR.json cargo bench -- overhead --nocapture`. Note that the vector choice doesn't matter much, because the Machine initialization procedure is identicall for all vectors.
- To get a perf flamegraph, run `CARGO_PROFILE_BENCH_DEBUG=true VECTOR=testing/conformance/test-vectors/corpus/REST_OF_TEST_VECTOR.json  cargo flamegraph --bench bench_conformance -- --nocapture`. The output SVG will be in `flamegraph.svg`.
- Overhead measurement scenarios. There are two overhead measurement scenarios included.
  1. `bench_init_only`: measure the overhead of running the benchmark itself, it doesn't send any messages to the FVM to process.
  2. `bench_500_simple_state_access`: measures the overhead of calling the `pubkey_address` method on an account actor 500 times, this is the most lightweight message possible to send that actually executes actor logic (unlike a bare send).

## Benchmark notes

**Build**

```shell
cargo build --release --bin perf-conformance
```

Note that unlike the tests and benchmarks, `perf-conformance` only expects a single test vector, not a directory. It is meant to be used with the [perf](https://man7.org/linux/man-pages/man1/perf.1.html) tool, as shown in the examples below.

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

## Visualize traces

The conformance tests support exporting traces for visualization. See under [measurements](./measurements/README.md).

## Adding new actor bundles

To add support for new actors releases, take the bundle [tar file from lotus](https://github.com/filecoin-project/lotus/tree/master/build/actors), add it to `testing/conformance/actors/`, and register it in `testing/conformance/src/actors.rs`.
