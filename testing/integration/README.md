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
