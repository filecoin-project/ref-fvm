# Changelog

Changes to the reference FVM implementation.

## Unreleased
 
- Added `testing` feature to change module visibility 

## 0.7.1 [2022-04-18]

This release adds support for execution traces in the FVM which can be enabled using the new `enable_tracing` option in the `MachineContext`.
The change is backwards compatible.

## 0.7.0 [2022-04-15]

This release contains exactly one (breaking) change.

BREAKING: Updates the FVM to the latest syscall struct alignment
(https://github.com/filecoin-project/fvm-specs/issues/63).

- Changed visibility of `account_actor`, `init_actor` and `system_actor` to public to use them in the integration test
framework.

## 0.6.0 [2022-04-13]

- WIP NV16 support.
- Implement [FIP0032][]: NV16 will now charge gas for more operations, including execution gas.
- BREAKING: Updates to fvm_shared 0.5.1
    - This refactors the exit code into a struct with constant values instead of an enum.
- BREAKING: Refactor the `Machine` constructor to take a `MachineContext` struct, reducing the
  number of parameters.
- BREAKING: Rename (internal) consume/take methods.
     - `BufferedBlockstore::consume` -> `BufferedBlockstore::into_inner`
     - `Executor::consume` -> `Executor::into_machine`
     - `Kernel::take` -> `Kernel::into_call_manager`
     - `Machine::consume` -> `Machine::into_store`
     - `Hamt::consume` -> `Hamt::into_store`
     - `StateTree::consume` -> `StateTree::into_store`
- BREAKING: remove unused (by the FVM) `verify_post_discount` from the FVM PriceList.

[FIP0032]: https://github.com/filecoin-project/FIPs/blob/master/FIPS/fip-0032.md