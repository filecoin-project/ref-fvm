# ref-fvm release/v3 (fvm 3)

This branch is FVM major 3. It runs network versions 18 to 20 (`SUPPORTED_VERSIONS` in `fvm/src/machine/default.rs`), which ran builtin-actors v10 and v11. The `nv21-dev` feature extends the range to 21, but hosts run 21 on fvm 4 and don't enable it. Lotus (through [filecoin-ffi](https://github.com/filecoin-project/filecoin-ffi)) and [Forest](https://github.com/ChainSafe/forest) use this branch to replay that finalised history, alongside fvm 2 and fvm 4 in the same build.

Active development happens on master. Its [`AGENTS.md`](https://github.com/filecoin-project/ref-fvm/blob/master/AGENTS.md) covers the engine, syscalls, gas, comments and review, and applies here except where this file differs.

## Maintenance only

- Every node must reproduce historical results exactly, so behaviour, gas and syscall changes don't land here. An execution bug on this branch is part of the chain.
- Accepted changes: dependency and security bumps, wasmtime moves with master, toolchain and CI updates, build fixes, and small additive APIs that hosts need. A dependency swap can still change execution; check what it touches.
- A vulnerability reachable only by replaying finalised messages is low severity, since nobody can add messages to that history.
- Backports arrive as separate PRs against this branch.

## Differences from master

- The engine matches master's layout (`fvm/src/engine/`). Keep the `fvm-wasm-instrument` version in `fvm/Cargo.toml`; changing it changes gas for every bundle. `accepted_wasm_features_are_fixed`, `effective_engine_config_is_fixed` and `padded_call_indirect_table_index_is_accepted` pin the accepted code and engine settings. If one fails after a bump, investigate; don't update the assertion. Keep `macos_use_mach_ports(false)`.
- Signature verification has no Cargo feature here. `m2-native` exists and CI checks that it compiles.
- The IPLD crates come from crates.io; they are released from master only.

## Shared crates

ffi and Forest resolve all three majors in one Cargo graph, so `wasmtime-*`, `cranelift-*`, `pulley-*`, `fvm_ipld_*`, `cid` and `multihash*` must resolve to the same versions here as on master and `release/v2`. Use `cargo update -p <crate> --precise <version>` on this branch rather than cherry-picking lockfile commits. `deny.toml` bans non-LTS wasmtime majors.

## Build and test

```sh
cargo fmt -- --check
make license
cargo clippy --all --all-targets --no-default-features -- -D warnings
cargo check --features=m2-native --no-default-features
cargo test --package fvm --no-default-features
cargo test --all --exclude fvm --exclude fvm_conformance_tests
git submodule update --init && cargo test --package fvm_conformance_tests
```

Conformance replays the network version 18 vectors (`0002-hygge-01`), skipping `DisputeWindowedPoSt/Ok` because the harness treats every PoSt as valid. It is this branch's main evidence that execution is unchanged. State what you ran.

## Releases

Add an entry under `[Unreleased]` in the changed crate's `CHANGELOG.md`. To release, a PR against this branch sets the versions with `cargo set-version` and dates the changelogs; the Release Checker dry-runs the publish. On merge, `releaser.yml` tags `crate@vX.Y.Z` and publishes to crates.io through trusted publishing. Hosts pin this branch's minor with `~`, so a minor release needs ffi and Forest bumps.
