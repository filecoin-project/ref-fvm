# EVM Contracts

This crate contains test [contracts](./contracts/) used for integration testing the FEVM in [fevm_features](../fevm_features).
When the project is compiled, the generated ABI and bytecode are written to the [artifacts](./artifacts/) directory.
The library generates ABI bindings as well, which can be imported into the tests.

`fevm.rs` can use [Cucumber](https://cucumber-rs.github.io/cucumber/current/quickstart.html) to run integration test scenarios,
for which the Gherkin feature specifications are in the [features](./features/) directory. Other than that, both libraries
should be fairly light on code.

After adding new contracts, the generated modules must be added to `lib.rs` manually (although this could be part of `build.rs`).

_Tip_: If it looks like a contract is not being picked up by `build.rs`, try `make all` to see if there's anything wrong with the Solidity compilation.
