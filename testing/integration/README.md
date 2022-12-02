# Integration test module

This directory contains tooling to run integration test over the FVM.

## Framework components

The bulk of the logic is handled by the `Tester` struct.

The following flow has been defined as a default usage:
1. Instantiate a new `Tester` specifying accessible accounts, the network and state tree versions.
2. (Repeat) Set new actor states and deploy given actor in the stand alone `Blockstore` and `StateTree`.
3. Interact with previously deployed actors by calling the `execute()` function.
> Note: Once the `execute()` is called new actors have to be instantiated with messages as the `Machine` and `Executor`
> are already instantiated
4. Make assertion on the `ApplyRet` of the message

## Current limitations

1. Wasm bytecode is now expected to be received through a binary type (`&[u8]`). This be upgraded to work Rust module compiled
at test time.
2. Some testing and examples should be added to demonstrate how the framework works.

TODO: (hack to get coverage reports from actors + integration tests)
```bash
cargo build -p "*actor"
export SKIP_WASM_BUILD=true
export FVM_STORE_ARTIFACT_DIR=../../target/llvm-cov-target/
cargo llvm-cov -p fvm_integration_tests --lcov
```

## Gas Calibration

The `./tests/fil_gas_calibration.rs` test doesn't test any specific rule; rather, it calls `./tests/fil-gas-calibration-actor`
with various parameters to exercise certain syscalls, while collecting gas metrics, on which it runs regressions to test if
the gas models we chose have a reasonable quality as estimators of execution time.

The way this is different than the metrics we collect under `conformance` tests in that we also capture the inputs,
so that we can estimate prices based on different input size for example, if that is our hypotheses. The `conformance` tests are
more about backtesting the gas model using the available test vectors, whereas here we are driving the data collection.

The traces and the regression results can be exported if the `OUTPUT_DIR` env var is specified.

For example:

```shell
OUTPUT_DIR=./measurements/out cargo test --test fil_gas_calibration
```
