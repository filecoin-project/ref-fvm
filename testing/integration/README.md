# Integration test module

This directory contains tooling to run integration test over the FVM.

## Framework components

The bulk of the logic is handled by the `Tester` struct.

The following flow has been defined as a default usage:

1. Instantiate a new `Tester` specifying accessible accounts, the network and state tree versions.
2. (Repeat) Set new actor states and deploy given actor in the stand alone `Blockstore` and `StateTree`.
3. Interact with previously deployed actors by calling the `execute()` function.
    > Note: Once the `execute()` is called new actors have to be instantiated with messages as the `Machine` and `Executor` are already instantiated
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

## Manually testing a builtin actor bundle

`custom_bundle_test` loads a CAR supplied at runtime and exercises its actors in
the FVM. It is marked `#[ignore]`, so normal tests and CI compile it but do not
execute it or require a local CAR.

From the ref-fvm workspace root:

```sh
FVM_ACTORS_BUNDLE=../builtin-actors/output/builtin-actors-mainnet.car \
cargo test --locked -p fvm_integration_tests --test custom_bundle_test \
  -- --ignored --nocapture
```

`FVM_ACTORS_BUNDLE` is required. Absolute paths are accepted; relative paths are
resolved from the ref-fvm workspace root, regardless of the directory from which
Cargo is invoked. The test prints the resolved path and reads the CAR directly.
No CAR is copied into the repository.

The test uses NV29 as its fixed execution baseline. There is no network-version
environment variable or development feature requirement. This checks execution
of the supplied code; it does not certify which network should install that
bundle.

Cargo also builds this package's existing dev-dependencies, including the stock
builtin bundle and test actors. The test itself executes the CAR from
`FVM_ACTORS_BUNDLE`. On macOS, the build requires a WASM-capable Clang; for a
Homebrew LLVM installation, prefix the command with
`CC=/opt/homebrew/opt/llvm/bin/clang`.

The test prints the CAR root, manifest entries and type IDs, message exit codes
and gas usage, and final state root. It validates manifest lookup consistency,
loads every listed WASM module, and checks:

- Account methods and automatic account creation.
- Placeholder creation and promotion to EthAccount.
- EAM deployment of an EVM contract that returns 42.
- Init.Exec creation of a multisig.
- Reward and power initialization through their WASM constructors, followed by
  Power.CreateMiner and a check of the miner's owner and worker addresses.

Miner creation uses the 32 GiB Window PoSt proof type supported by mainnet and
calibration bundles, and supplies 100 FIL. Bundles with different sector policies
may need a different proof type in the test.

### Optional verified registry and DataCap checks

`FVM_CHECK_FILPLUS=1` requires both `verifiedregistry` and `datacap` in the
manifest. The test initializes both through their WASM constructors and checks
real method results: Verifreg.GetClaims reports a missing claim in its batch
result, DataCap.Name returns `DataCap`, Balance returns zero, and Mint from a
non-governor returns `USR_FORBIDDEN` (18).

```sh
FVM_ACTORS_BUNDLE=/absolute/path/bundle-with-filplus.car \
FVM_CHECK_FILPLUS=1 \
cargo test --locked -p fvm_integration_tests --test custom_bundle_test \
  -- --ignored --nocapture
```

To test a bundle with those actors removed, also set
`FVM_EXPECT_FILPLUS_MISSING=1`. Both actor types must be absent from the manifest;
the same calls must return `SYS_INVALID_RECEIVER` (5), and both actors must remain
absent afterward.

```sh
FVM_ACTORS_BUNDLE=../builtin-actors/output/builtin-actors-mainnet.car \
FVM_CHECK_FILPLUS=1 \
FVM_EXPECT_FILPLUS_MISSING=1 \
cargo test --locked -p fvm_integration_tests --test custom_bundle_test \
  -- --ignored --nocapture
```

| Supplied bundle | Check enabled, missing flag unset/false | Both flags true |
| --- | --- | --- |
| Both Fil+ types present | Verify constructors and method results | Fail: actors unexpectedly present |
| Both Fil+ types absent | Fail: required actors missing | Verify missing-receiver errors |

Both switches accept `1`/`true` or `0`/`false` and are false when unset. Invalid
values fail the test. `FVM_EXPECT_FILPLUS_MISSING=1` requires
`FVM_CHECK_FILPLUS=1`; it cannot silently enable or skip checks. With neither flag
set, only the common bundle loading and execution checks run.

Actor code CIDs come from the supplied manifest; no fixed actor count or
FIP-specific manifest layout is required. The test uses a fresh state,
DummyExterns, and zero fees. It does not test migration, activation scheduling,
proof verification, full-chain execution, or every method of every actor. The
optional checks do not establish that an existing chain's actors were removed.
