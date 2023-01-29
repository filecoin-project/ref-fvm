# fvm-bench
Tools for testing and benchmarking FVM

## Status

Currently this is a program called `fvm-bench`, which allows you to execute and gas-benchmark
fevm contracts.

This is a barebones MVP, but it is the only program we have that can execute evm contracts with fvm.

Usage:
```
Run a contract invocation for benchmarking purposes

Usage: fvm-bench [OPTIONS] --bundle <BUNDLE> <CONTRACT> <METHOD> <PARAMS>

Arguments:
  <CONTRACT>  Contract file
  <METHOD>    Invocation method; solidity entry point for fevm, actor method for wasm
  <PARAMS>    Invocation parameters, in hex

Options:
  -m, --mode <MODE>            Execution mode: wasm or fevm [default: fevm]
  -d, --debug                  Emit debug logs
  -t, --trace                  Emit detailed gas tracing information
  -e, --events                 Emit user generated logs
  -b, --bundle <BUNDLE>        Builtin actors bundle to use
  -g, --gas-limit <GAS_LIMIT>  Gas limit in atto precision to use during invocation. Default: 10 billion gas [default: 10000000000]
  -h, --help                   Print help
```

Example invocations:
```
$ ../../target/release/fvm-bench -b ~/src/fvm/builtin-actors/output/builtin-actors.car ../contracts/benchmarks/empty.bin "" ""
Result:
Gas Used: 1364997

$ ../../target/release/fvm-bench -b ~/src/fvm/builtin-actors/output/builtin-actors.car ../contracts/benchmarks/SimpleCoin.bin f8b2cb4f 000000000000000000000000ff00000000000000000000000000000000000064
Result: 0000000000000000000000000000000000000000000000000000000000002710
Gas Used: 1764645
```
