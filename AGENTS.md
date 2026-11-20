# ref-fvm release/v2 (fvm 2)

This branch is FVM major 2. It accepts network versions 15 to 17 (`SUPPORTED_VERSIONS` in `fvm/src/machine/default.rs`); hosts use it for 16 and 17, which ran builtin-actors v8 and v9. Lotus (through [filecoin-ffi](https://github.com/filecoin-project/filecoin-ffi)) and [Forest](https://github.com/ChainSafe/forest) use it to replay that finalised history, alongside fvm 3 and fvm 4 in the same build.

Active development happens on master. Its [`AGENTS.md`](https://github.com/filecoin-project/ref-fvm/blob/master/AGENTS.md) covers the engine, syscalls, gas, comments and review, and applies here except where this file differs.

## Maintenance only

- Every node must reproduce historical results exactly, so behaviour, gas and syscall changes don't land here. An execution bug on this branch is part of the chain.
- Accepted changes: dependency and security bumps, wasmtime moves with master, toolchain and CI updates, build fixes, and small additive APIs that hosts need. A dependency swap can still change execution: moving secp256k1 recovery to `k256` rejected high-s signatures until a918eff9 accepted them again. Check what a swap touches.
- A vulnerability reachable only by replaying finalised messages is low severity, since nobody can add messages to that history.
- Backports arrive as separate PRs against this branch.

## Differences from master

- The engine is in `fvm/src/machine/engine.rs`, and the metering instrumenter is `fvm-wasm-instrument` 0.2.0. Changing the instrumenter version changes gas for every bundle; keep it.
- v8 and v9 bundles use the minimal `call_indirect` encoding. `fvm-wasm-instrument` 0.2 rejects the padded form (`padded_call_indirect_table_index_is_rejected_on_load`), unlike master.
- `accepted_wasm_features_are_fixed` and `effective_engine_config_is_fixed` pin the accepted features and engine settings. If one fails after a bump, investigate; don't update the assertion. Keep `macos_use_mach_ports(false)`.
- Signature verification has no Cargo feature here, and `m2-native` doesn't exist.
- The IPLD crates come from crates.io; they are released from master only.

## Shared crates

ffi and Forest resolve all three majors in one Cargo graph, so `wasmtime-*`, `cranelift-*`, `pulley-*`, `fvm_ipld_*`, `cid` and `multihash*` must resolve to the same versions here as on master and `release/v3`. Use `cargo update -p <crate> --precise <version>` on this branch rather than cherry-picking lockfile commits. `deny.toml` bans non-LTS wasmtime majors.

## Build and test

```sh
cargo fmt -- --check
cargo clippy --all --all-targets --no-default-features -- -D warnings
cargo test --package fvm --no-default-features
cargo test --all --exclude fvm --exclude fvm_conformance_tests
git submodule update --init && cargo test --package fvm_conformance_tests
```

Conformance replays the network version 17 vectors (`0001-shark-01`) and is this branch's main evidence that execution is unchanged. State what you ran.

## Releases

Add an entry under `[Unreleased]` in the changed crate's `CHANGELOG.md`. To release, a PR against this branch sets the versions with `cargo set-version` and dates the changelogs; the Release Checker dry-runs the publish. On merge, `releaser.yml` tags `crate@vX.Y.Z` and publishes to crates.io through trusted publishing. Hosts pin `~2.13`, so a minor release needs ffi and Forest bumps.
