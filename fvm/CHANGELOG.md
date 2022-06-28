# Changelog

Changes to the reference FVM implementation.

## Unreleased

...

## 1.1.0 [2022-06-27]

- debug execution: implement actor redirects in engine

## 1.0.0 [2022-06-23]

- Fixup WASM sections after instrumenting for gas and stack accounting. Without this,
  instrumentation would produce incorrect wasm modules in some cases.
- Fix exec tracing when stack depth is exceeded.
- Fix logging syscall to skip logging when debugging is _not_ enabled (the check was flipped).
- Audit and cleanup TODOs.
- Remove unused imports, etc.
- Refactor the blockstore "flush" to behave more like lotus.
- Upgrade wasmtime to 0.37.
- Fix the read syscall to correctly compute the returned "offset". Previously, it would never return
  a negative value, even if the passed-in buffer was over-sized.
- Make `DefaultExecutor#flush` a method on the `Executor` trait.
- Catch additional inner panics at the kernel layer, lowering them to syscall errors.
- General refinement of error handling by returning more fitting error numbers.
- General upstream dependency upgrade, including Wasmtime to 0.36.0.
- Reinstate the MAX_CID_LEN of 100 bytes.

## 0.8.0 [2022-05-16]

This release includes several major features:

- Final nv16 gas numbers (including charges for memcopies, extern calls, syscalls, etc.).
- Significantly improved wasm gas accounting, and stack accounting, through wasm instrumentation.
- A `ThreadedExecutor` for executing messages on a new thread. This is necessary because we need at
  least 64MiB of stack.
- Panics are now caught at every sub-call and turned into fatal errors.
- When a fatal error is encountered, we now allow the network to continue by:
  - Consuming all message gas.
  - Failing the entire _message_, but not the block.
- A large syscall refactor. These syscall interfaces should be the _final_ interfaces for M1.

**Breaking TL;DR:**

- This release DROPS SUPPORT for nv14.
- This release REQUIRES builtin-actors v7.4.x. v7.3.x _will not work_ due to breaking syscall changes.
- Users _must_ wrap the `DefaultExecutor` in a `ThreadedExecutor` unless they can otherwise
  guarantee at least 64MiB of stack.
- The execution trace format has changed.

Additionally, this release includes:

- Strongly typed a `Gas` type to help statically catch and prevent bugs in gas math.
- Refactored syscalls as described in the `fvm_sdk` v0.7.0 changelog.
- An audited and cleaned up wasmtime config.
- A smaller recursive call limit (now 1024 recursive sub-calls and 2k wasm stack elements).
- Drops support for NV14.
- Requires builtin-actors v7.4.x

## 0.7.2 [2022-05-09]
 
- Add `testing` feature to change module visibility; concretely changed
  visibility of `account_actor`, `init_actor` and `system_actor` to `pub`
  to use them in the integration test framework.
- Propagate gas outputs in ApplyRet.
- Migrate CBOR serde to [cbor4ii](https://github.com/quininer/cbor4ii).
- Instrument Wasm bytecode with [filecoin-project/fvm-wasm-instrument](https://github.com/filecoin-project/fvm-wasm-instrument), 
  a fork of [paritytech/wasm-instrument](https://github.com/paritytech/wasm-instrument)
  for more accurate stack accounting and execution units metering.
- Abort when aborting fails.
- Fix syscall binding docs. 
- Fix bugs in Wasm execution units gas accounting.
- Fix system actor state serialization.
- Remove unused dependencies from build graph.
- Optimize memory resolution so it only happens once.

## 0.7.1 [2022-04-18]

This release adds support for execution traces in the FVM which can be enabled using the new `enable_tracing` option in the `MachineContext`.
The change is backwards compatible.

## 0.7.0 [2022-04-15]

This release contains exactly one (breaking) change.

BREAKING: Updates the FVM to the latest syscall struct alignment
(https://github.com/filecoin-project/fvm-specs/issues/63).

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
