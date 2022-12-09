# Test vector runner and benchmarker

This directory contains tooling to run test vectors against the FVM in the form
of tests and benchmarks.

## Link `test-vectors`

The instructions below assume that the https://github.com/filecoin-project/fvm-test-vectors repo is checked out as a sibling directory next to `ref-fvm`, and then in the `conformance` directory we have created the following symlink:

```shell
ln -s ../../../fvm-test-vectors test-vectors
```

## Instructions

- To run all tests, just run `cargo test`.
- To run all test vectors under a specific directory, run eg. `VECTOR=test-vectors/corpus/extracted cargo test conformance -- --nocapture`
- To run a specific test vector, run `VECTOR=test-vectors/corpus/specs_actors_v7/REST_OF_TEST_VECTOR.json cargo test -- conformance --nocapture`
- To bench a specific test vector, run `VECTOR=test-vectors/corpus/specs_actors_v7/REST_OF_TEST_VECTOR.json cargo bench -- conformance --nocapture`
- To bench the system's overhead for the setup of the machine for a given test vector, run `VECTOR=test-vectors/corpus/specs_actors_v7/REST_OF_TEST_VECTOR.json cargo bench -- overhead --nocapture`. Note that the vector choice doesn't matter much, because the Machine initialization procedure is identicall for all vectors.
- To get a perf flamegraph, run `CARGO_PROFILE_BENCH_DEBUG=true VECTOR=testing/conformance/test-vectors/corpus/specs_actors_v7/REST_OF_TEST_VECTOR.json  cargo flamegraph --bench bench_conformance -- --nocapture`. The output SVG will be in `flamegraph.svg`.
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
  VECTOR=testing/conformance/test-vectors/corpus/specs_actors_v7/TestMeasurePreCommitGas/ffbecb89ee7d7847d104bd237f90e5139d4fb32c49a46e8e74718e34886bebda-t0100-t0101-storageminer-25.json \
  ./target/release/perf-conformance
```

**Run benchmark**

For a single vector:

```shell
CARGO_PROFILE_BENCH_DEBUG=true \
  VECTOR=testing/conformance/test-vectors/corpus/specs_actors_v7/TestMeasurePreCommitGas/ffbecb89ee7d7847d104bd237f90e5139d4fb32c49a46e8e74718e34886bebda-t0100-t0101-storageminer-25.json \
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
