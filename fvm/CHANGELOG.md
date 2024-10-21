# Changelog

Changes to the reference FVM implementation.

## [Unreleased]

## 4.4.3 [2024-10-21]

- Update wasmtime to 25.0.2.
- Fixes long wasm compile times with wasmtime 24.

## 4.4.2 [2024-10-09]

- Update wasmtime to 24.0.1.

## 4.4.1 [2024-10-04]

- chore: remove the `nv24-dev` feature flag [#2051](https://github.com/filecoin-project/ref-fvm/pull/2051)

## 4.4.0 [2024-09-12]

- Update to wasmtime 24.
- Switch from mach ports to unix signal handlers on macos.
- Update misc dependencies.

## 4.3.2 [2024-08-16]

- feat: add `nv24-dev` feature flag [#2029](https://github.com/filecoin-project/ref-fvm/pull/2029)

## 4.3.1 [2024-06-26]

- **BREAKING**: Simplify the verify-signtures feature and update ambassador. This is a minor-breaking change because the ambassador macros are now only exported from the prelude/kernel module, not the crate root as they previously were.
- chore: remove the `nv23-dev` feature flag [#2022](https://github.com/filecoin-project/ref-fvm/pull/2022)
- chore: update wasmtime to 19.0.2

## 4.3.0 [2024-06-12]

- feat: FIP-0079: syscall for aggregated bls verification [#2003](https://github.com/filecoin-project/ref-fvm/pull/2003)
- fix: install rust nightly toolchain for clusterfuzzlite [#2007](https://github.com/filecoin-project/ref-fvm/pull/2007)
- chore: upgrade rust toolchain to 1.78.0 [#2006](https://github.com/filecoin-project/ref-fvm/pull/2006)
- fix: remove the pairing feature from fvm_shared [#2009](https://github.com/filecoin-project/ref-fvm/pull/2009)
- Small tidy-ups in CONTRIBUTING.md [#2012](https://github.com/filecoin-project/ref-fvm/pull/2012)
- NI-PoRep support [#2010](https://github.com/filecoin-project/ref-fvm/pull/2010)

## 4.2.0 [2024-04-29]

- chore: update to wasmtime 19.0.1 [#1993](https://github.com/filecoin-project/ref-fvm/pull/1993)
- Enable nv23 support behind the `nv23-dev` feature flag [#2000](https://github.com/filecoin-project/ref-fvm/pull/2000)
- feat: fvm: remove once_cell [#1989](https://github.com/filecoin-project/ref-fvm/pull/1989)
- feat: shared: check bls zero address without lazy_static [#1984](https://github.com/filecoin-project/ref-fvm/pull/1984)

## 4.1.2 [2024-01-31]

feat: allow CBOR events

## 4.1.1 [2024-01-25]

Enable nv22 support by default.

## 4.1.0 [2024-01-24]

- Default the concurrency of the `ThreadedExecutor` to the available parallelism instead of 8.
- Support custom syscalls (only needed for non-Filecoin users).
    - Filecoin now uses the `FilecoinKernel`, not the `DefaultKernel`. The `DefaultKernel` no longer implements Filecoin specific features.
    - The `Kernel` trait now inherits from the `SyscallHandler` trait, allowing kernels to decide how they want to expose themselves to actors via syscalls.
- Many internal architecture cleanups with respect to the Kernel and syscall bindings.
- Added the current actor state to the execution trace (in the `Invoke` event).
- Add a syscall (enabled with the "upgrade-actor" feature flag) that lets actors "swap-out" their code-CID.

## 4.0.0 [2023-10-31]

Final release, no changes.

## 4.0.0-alpha.4 [2023-09-28]

- Add back some proof types that were mistakenly removed, and fix some of the constants.

## 4.0.0-alpha.3 [2023-09-27]

- Remove support for v1 proofs.
- Make it possible to construct a GasDuration (and make it possible to refer to the GasDuration type).

## 4.0.0-alpha.2 [2023-09-21]

- Update to wasmtime 12.0.2 (bug fix release)
- Drop support for versions prior to nv21.
- Implement FIP-0071, FIP-0072, FIP-0073, FIP-0075

## 4.0.0-alpha.1 [2023-09-20]

Unreleased. This release simply marks the change-over to v4.

## 3.8.0 [2023-09-06]

- Upgrade wasmtime to v12. Unlike prior wasmtime upgrades, this shouldn't be a breaking change as it now mangles its symbols.
- BREAKING: Upgrade the proofs API to v16.
- BREAKING (linking): upgrade blstrs to v0.7 and
- BREAKING: update the minimum rust version to 1.70.0

## 3.7.0 [2023-08-28]

- BREAKING: Add fields to the trace API:
    - Add a `gas_limit` and `read_only` fields to `ExecutionEvent::Call`.
    - Add a new `ExecutionEvent::InvokeActor(code_cid)` event when an actor is invoked on any method number (including method 0).
- Remove the unused `crypto::verify_seal` syscall.

## 3.6.0 [2023-08-18]

Breaking Changes:
- Perform randomness hashing in the kernel
  - The FVM no longer supplies a DST and entropy to the client extern when requesting randomness
  - It expects the client to return the "digest", from which the kernel then draws the randomness
  - Clients integrating this change should:
    - no longer expect the DST and entropy parameters for the `get_chain_randomness` and `get_beacon_randomness` externs
    - omit the last step they currently perform when drawing randomness; that is, return the hashed digest after looking up the randomness source
- Drop deprecated `hyperspace` feature

Other Changes:
- Add support for nv21 behind the `nv21-dev` feature flag
  - Note: We do NOT expect to support nv21 on FVM3, this only facilitates the development of nv21 before FVM4 is ready
  - We use the Hygge pricelist as-is for nv21 if the `nv21-dev` feature flag is enabled
- Refactor: remove `with_transaction` and move the "return" gas charge
  - transaction logic is now entirely in `CallManager::send`
- Syscalls: fix: Do not assume return pointers are aligned
- Buffered Blockstore: fixup IPLD flush logic 
  - Make it less generic (performance).
  - Remove blocks from the write buffer as we write them to avoid
    duplicate writes.
  - Simplify some of the checks around what is allowed. For example, I'm
    now allowing CBOR + Identity hash which should have been allowed
    previously but wasn't (we don't use it but still, it should have been
    allowed).
  - Remove the explicit 100 byte CID length check. The `Cid` type already
    validates that the digest can be no longer than 64 bytes.
  - Be less strict on DagCBOR validation. Counter-intuitively, being
    overly strict here is dangerous as it gives us more points where
    implementations can disagree and fork. Instead, we enforce the
    following rules for DAG_CBOR:
    1. Blocks must have a valid CBOR structure, but _values_ aren't
       validated. E.g., no utf-8 validation, no float validation, no
       minimum encoding requirements, no canonical ordering requirements,
       etc.
    2. All CBOR values tagged with 42 must be valid CIDs. I.e., a CBOR
       byte string starting with a 0x0 byte followed by a valid CID with
       at most a 64 byte digest.

## 3.5.0 [2023-06-27]

Breaking Changes:

- Update cid/multihash. This is a breaking change as it affects the API.
- Make `Kernel::send` generic over the kernel that will be used by the called actor. This significantly simplifies testing.
- Upgrade wasmtime to v10.

Other Changes:

- This version allows over-subscription on the instance pool when executing multiple messages in parallel. This means the number of parallel execution lanes is no longer limited by the address space (or no longer _as_ limited).

## 3.4.0 [2023-05-04]

Update wasmtime to 8.0.1. This is a breaking change if you use any other wasmtime version.

## 3.3.1 [2023-04-26]

This release contains a small hack for calibrationnet and is optional for users who only want to
sync mainnet.

## 3.3.0 [2023-04-23]

- Fixes an issue with proof bindings.

## 3.2.0 [2023-04-04]

- Support nv19/nv20
- Remove unused dependencies.
- BREAKING: Upgrade proofs to version 14.0.0 (for nv19).
- Switch build to rust stable (finally).

## 3.1.0 [2023-03-09]

Update proofs. Unfortunately, this is a breaking change in a minor release but we need to do the same on the v2 release as well. The correct solution is to introduce two crates, fvm1 and fvm2, but that's a future project.

## 3.0.0 [2023-02-24]

- Final release for NV18.

## 3.0.0-rc.1 [2022-02-13]

- Removes an incorrect event size limit.

## 3.0.0-alpha.24 [2022-02-09]

- Add IPLD codecs to the gas trace. I.e., use `IpldBlock` instead of `RawBytes`.
- Finalize gas parameters.

## 3.0.0-alpha.23 [2022-02-06]

- Large update to gas charging:
    - Change send gas.
    - Add actor lookup/update gas.
    - Add address lookup/update gas.
    - Update IPLD gas fees.
    - Update event gas.
    - Add a tipset lookup gas fee.
- Tweaks to the event syscall parameters (it now takes a codec and accepts raw values).

## 3.0.0-alpha.22 [2022-02-01]

- Align events implementation with FIP-0049 (#1481)
- feat: explicitly reject placeholder creation (#1568)
- Integrate fvm-bench and the basics of a testkit (#1493)
- feat: simplify gas tracking stack (#1526)
- feat: `CarReader::read_into()` (#1524)
- feat: normalize transaction signatures (#1525)
- fix: expose the effective gas premium (#1512)

## 3.0.0-alpha.21 [2022-01-19]

- Machine: Put the Empty Array object in the blockstore on creation
- Kernel: Restrict `create_actor` to the InitActor
  - We make an exception for integration tests
- Deps: Update `derive_builder` to 0.12.0
- Use CBOR instead of DAG_CBOR for message params

## 3.0.0-alpha.20 [2022-01-17]

- Add `hyperspace` feature to loosen up network version restrictions.

## 3.0.0-alpha.19 [2022-01-13]

- Adjust memory retention gas fees.
- Add a basic block-size limit of 1MiB.
- Update wasmtime to v2.0.2

## 3.0.0-alpha.18 [2022-01-10]

- Remove the CBOR trait
  - the `read_cbor` syscall is implemented over `DeserializeOwned`
- Executor: Always transform embryo to eth_account if executing message
- Rename embryo -> placeholder
- Kernel: remove support for non-key addresses from `verify_signature`
- Gas: Make the block "read" charge more accurate 
- StateTree: Rewrite snapshotting to have O(1) lookups
  - Maintain an undo history instead of true state "layers"
- Kernel: fix: return `NotFound` from `balance_of`
- feat: refactor memory limits and apply to tables
  - Refactors memory limits to remove the per-instance limits from the limiter itself
  - Removes wasmtime interfaces from the limiter, instead implementing a wrapper\
- Gas: Finalize write costs
- Gas: Remove the explicit extern cost
- CallManager: Change the recursive call limit to 1024

## 3.0.0-alpha.17 [2022-12-19]

- feat: split the state-tree's "read" and "write" caches
- fix: use correct sender state for account abstraction
- refactor: update the gas schedule:
  - Remove the old gas schedules.
  - Use ScalingCost everywhere to make things simpler.
  - Try to clearly split up operations in a way that should be easier to measure.
  - Rename the "storage gas" field on gas charges to "other gas", and move all charges that aren't charging for immediate computation. This makes benchmarking easier. Includes:
    - Deferred operations (e.g., flush).
    - Memory retention
    - Storage
  - Signature Validation:
    - Secp256k1 signature verification has gained a new 10gas/byte cost to reflect the cost of hashing (blake2b).
    - BLS signature verification has gained a new 26gas/byte cost to reflect the cost of hashing.
  - Hashing no longer has a flat cost (was 31355) but has the following costs (per algorithm):
    - sha256: 7gas/byte
    - blake2b: 10gas/byte
    - keccak256: 33gas/byte
    - ripemd160: 35gas/byte
  - Memory:
    - Memory copy costs have been reduced from 0.5gas/byte to 0.4gas/byte.
    - The "memory retention" cost of 10gas/byte has been split into a 2gas/byte memory _allocation_ cost, and an 8gas/byte memory retention cost.
  - Storage:
    - Block storage costs have increased by 13.8gas/byte (from 1300gas/byte to 1313.8gas/byte, or 1%):
      - PLUS 2 * 2 = 4 gas for an expected 2 allocations (one on write, one on flush).
      - PLUS 10gas/byte for hashing.
      - MINUS 0.1 * 2 gas/byte for the reduction in memcpy costs.
  - Randomness now charges for hashing:
    - 1400 for the "extern" call. 
    - 10gas/byte of entropy plus 480 gas for hashing the randomness itself.
- Explicit gas charges for different instruction types
- feat: charge 0.4gas/byte for memory copy and initialization

## 3.0.0-alpha.16 [2022-12-17]

- fix: remove duplicate "create_actor" method
- chore: remove "singleton" check
  - The singletons now generally assert that they've been created by the "system" actor.
- feat: only store delegated addresses in the state-tree
  - Restore the logic for resolving key addresses when verifying signatures.
  - Mass rename of predictable -> delegated.
  - No longer store f1/f3 in the delegated_address field of an ActorState.
- CallManager: Restrict embryo creation to the EAM's namespace
- feat: Gas: Reprice syscalls for which we have models

## 3.0.0-alpha.15 [2022-12-14]

- Refactor: Extract the `Engine` from the `Machine` and make it a pool
  - The `Engine` is replaced by an "Engine Pool", with `concurrency * call_depth` instances
  - The Engine Pool lives in the `Executor`
  - The `Engine` itself lives in the `CallManager`
- Update instrumentation logic
- Add charging logic for all memory copy and init operations
- Refactor: Move `ChainID` out of FVM (and into shared)
- Compile with `m2-native`
- Fix: Missing `Engine` getter 
- Feat: Gas timing stats and visualization
  - Adds gas timing tracing to conformance tests 
  - Adds a gas calibration contract to run specific instructions
- Feat: Implement Ethereum Account Abstraction
  - Remove the f4-as-account feature/hack entirely
  - The executor checks if sender is an embryo actor
  - If so, and the delegated address is in the f410 namespace, an Ethereum Account is deployed there

## 3.0.0-alpha.14 [2022-12-08]

- In send, change 0 gas to mean 0 gas (not unlimited).

## 3.0.0-alpha.13 [2022-12-07]

- FIX: Only push backtrace frames on _error_.
- Remove the gas limit from the context.
- Disable `memory_init_cow` in wasmtime. This will use a bit more memory, but will be predictable.
- Add the chain ID to the network context (defaults to 0).
- Add the nonce to the message context.

## 3.0.0-alpha.12 [2022-11-29]
- Fix: make sure exit never fails, even on an invalid message
- Limit the size of backtrace messages to 1k
- Add interior mutability to the gas tracker
- Refactor: Use ActorIDs for internal methods instead of Addresses
  - The caller now has to handle any resolution errors, and can choose how to do so
  - Avoid marking failures in some state-tree modification functions as "fatal errors".
- Add a read-only mode to Sends
  - Adds the concept of "read-only" layers to both the event accumulator and the state tree.
- Raise the maximum memory limit from 512MiB to 2GiB
- Fix: charge for the first page of gas
- Send syscall: add an optional gas limit
  -If specified, this limit will restrict the Send, not the message's remaining gas limit
- Charge gas for more syscalls
  - See [#1139](https://github.com/filecoin-project/ref-fvm/pull/1139) for the complete list

## 3.0.0-alpha.11 [2022-11-18]

- Replace `new_actor_address` with `next_actor_address`. `next_actor_address` has no side effects (until the actor is actually created).
- Change `next_actor_address` to always use the origin address from the message, as specified. For abstract accounts, we _can't_ lookup a key address (they may only have an f0 and f2 address).
- Move account creation logic to the call manager.
  - The call manager owns the relevant state.
  - The call manager will eventually invoke the constructor directly when creating the actor.
- Change the `abort` syscall to `exit` to allow:
  - non-local exits.
  - returning values on aborts.
- Add a method to the externs to lookup tipset CIDs.
- Remove the NetworkContext from the FVM builder API because we no longer expect the user to pass us a list of tipset CIDs.
- Change kernel internals to merge all network/message "context" methods into single methods returning `*Context` structs.
- Avoid treating out of memory instantiation errors as fatal.

## 3.0.0-alpha.10 [2022-11-17]

- Refactor network/message contexts to reduce the number of syscalls.

## 3.0.0-alpha.9 [2022-11-16]

- fix: BufferedBlockstore#flush should not reset the write buffer.

## 3.0.0-alpha.8 [2022-11-15]

- Add support for actor events (FIP-0049).

## 3.0.0-alpha.7 [2022-11-14]

- MEM-851: Memory expansion gas (#1067)
- Split `InvokeContext` into two (#1070)
- Support EAM singleton in manifest (#1005)

## 3.0.0-alpha.6

- update the state-tree version to v5
- enable instrumentation of sign extension instructions (only relevant to anyone playing around with native actor support).

## 3.0.0-alpha.5

- fix compile issues with f4-as-account feature.

## 3.0.0-alpha.4

- Resolve key addresses from the state tree instead of reaching into the account actor state
- Temporary workaround: allow validating signatures from embryo f4 addresses

## 3.0.0-alpha.3

- Fix the address length checks in the `create_actor` syscall. The previous release was broken.

## 3.0.0-alpha.2

- Autoload wasm modules from the blockstore if they haven't been preloaded.
- Add a new `balance_of` syscall.
- Add a new `tipset_cid` syscall.
- Add a new `timestamp` syscall.
- Add syscalls to get the gas limit, premium, and available gas.
- Add support for f4 addresses and auto-creating "embryos" on first send to an f4 address.
- Update wasmtime to 1.0.
- Add support for network version 18.

## 3.0.0-alpha.1

- Add the origin to the `vm::context` syscall.
- Add an `m2-native` feature to enable native actor deployment.

## 2.0.0...

See `release/v2`

- Added `recover_secp_public_key` syscall
- API BREAKING: Change `TokenAmount` type from a newtype to a struct.
- Add support for additional hash functions in actors:
    - sha256
    - keccak256
    - ripemd160
    - blake2b512
- API BREAKING: add gas charges to the execution trace.

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
