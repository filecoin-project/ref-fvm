# fvm-bench
Tools for testing and benchmarking FVM

## Status

Currently this is a program called `fvm-bench`, which allows you to execute and gas-benchmark
fevm contracts.

This is a barebones MVP, it requires you to have checked out ref-fvm in a sibling directory
in order to build.

Usage:
```
Run a contract invocation for benchmarking purposes

Usage: fvm-bench [OPTIONS] --bundle <BUNDLE> <CONTRACT> <METHOD> <PARAMS>

Arguments:
  <CONTRACT>  Contract file
  <METHOD>    Invocation method; solidity entry point for fevm, actor method for wasm
  <PARAMS>    Invocation parameters, in hex

Options:
  -m, --mode <MODE>      Execution mode: wasm or fevm [default: fevm]
  -b, --bundle <BUNDLE>  Builtin actors bundle to use
  -h, --help             Print help information
```

Example invocation:
```
$ ./target/debug/fvm-bench -b bundles/builtin-actors-next-3c902469.car ../builtin-actors/actors/evm/tests/contracts/SimpleCoin.bin f8b2cb4f 000000000000000000000000ff00000000000000000000000000000000000064
Contract invocation successfull
Result: 0000000000000000000000000000000000000000000000000000000000002710
Gas Used: 2290168

$ ./target/debug/fvm-bench -b bundles/builtin-actors-next-5a4b15b9.car ../builtin-actors/actors/evm/tests/contracts/SimpleCoin.bin f8b2cb4f 000000000000000000000000ff00000000000000000000000000000000000064
Contract invocation successfull
Result: 0000000000000000000000000000000000000000000000000000000000002710
Gas Used: 2252508
```
