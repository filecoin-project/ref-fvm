# ref-fvm agent guide

ref-fvm is the Rust reference implementation of the Filecoin Virtual Machine. Every Filecoin node runs it to execute builtin actors, so its execution results and gas charges are consensus.

Current code, CI and the linked repository documentation are authoritative.

## Where ref-fvm sits in Filecoin

- [builtin-actors](https://github.com/filecoin-project/builtin-actors) compiles against `fvm_sdk`, `fvm_shared` and the `fvm_ipld_*` crates and ships the WASM bundles the FVM executes. The FVM runs only builtin actor code: user contracts run inside the builtin EVM actor, and native user actors (`install_actor`) exist only behind the `m2-native` feature, which ffi doesn't enable. EVM semantics live in builtin-actors; ref-fvm supplies the syscalls the EVM actor calls.
- [filecoin-ffi](https://github.com/filecoin-project/filecoin-ffi) links fvm2, fvm3 and fvm4 into one binary for Lotus and Curio, selects the major by network version (`rust/src/fvm/engine.rs`, where `EngineVersion::V1/V2/V3` means fvm 2/3/4), implements `Externs` by calling back into Go, and maps each major's trace and receipt types to its own; a new `ExecutionEvent` variant is dropped from Lotus traces until ffi maps it. Lotus runs its legacy Go VM before network version 16 and ref-fvm through ffi from 16 on.
- [Forest](https://github.com/ChainSafe/forest) depends on the fvm2, fvm3 and fvm4 crates directly and selects one by network version as ffi does (`src/interpreter/vm.rs`), so a `release/v2` or `release/v3` change reaches Forest too. Forest can't execute state from before network version 16, which Lotus replays on its Go VM. Lotus and Forest must compute identical results from the same crates.
- Proof-verification syscalls call [rust-fil-proofs](https://github.com/filecoin-project/rust-fil-proofs) through `filecoin-proofs-api`; the default `opencl` feature comes from there.
- `ipld/hamt`, `ipld/amt` and `ipld/kamt` must match [go-hamt-ipld](https://github.com/filecoin-project/go-hamt-ipld), [go-amt-ipld](https://github.com/filecoin-project/go-amt-ipld) and [go-kamt-ipld](https://github.com/filecoin-project/go-kamt-ipld) bit for bit.
- [actors-utils](https://github.com/filecoin-project/actors-utils) builds FRC libraries on `fvm_sdk` with default features, which builtin-actors also consumes.

Consumers pin the primary crates to a minor with `~`, so every minor release needs a bump in ffi, Forest, builtin-actors and actors-utils. When a change crosses one of these boundaries, check the other side. If a sibling checkout is available, read its `AGENTS.md` before changing a shared contract.

### Versions and branches

Each major executes a fixed range of network versions and the builtin-actors bundles that ran there:

| fvm | branch | network versions | actors |
|---|---|---|---|
| 2 | `release/v2` | 15-17 accepted, used by hosts from 16 | v8-v9 |
| 3 | `release/v3` | 18-20 | v10-v11 |
| 4 | `master` | 21 onward | v12 onward |

Every bundle that has run on mainnet or calibnet must keep loading and producing the same results under its major, including calibnet's superseded code (listed in Lotus `build/builtin_actors.go`). To test against them, Lotus embeds every bundle from v8 in `build/actors/v*.tar.zst`, one CAR per network, and builtin-actors attaches `builtin-actors-<network>.car` to each GitHub release. Integration tests here use only builtin-actors master. `release/v2` and `release/v3` replay finalised history and change only through maintenance backports; see Backports and crate coexistence.

### Adding a network version

The next network version sits behind an `nv<N>-dev` Cargo feature on `fvm` until it is scheduled. Adding one touches the `NetworkVersion` constant (`shared/src/version/mod.rs`), the `SUPPORTED_VERSIONS` pair in `fvm/src/machine/default.rs`, the `price_list_by_network_version` arm (`fvm/src/gas/price_list.rs`), the feature in `fvm/Cargo.toml` and both changelogs. ffi then extends its fvm4 range in `rust/src/fvm/engine.rs` and its feature list. Promoting it removes the feature and its `cfg` pairs.

## Orientation

- `fvm/` is the VM. `Machine` (`machine/`) is anchored at a state root, epoch and `NetworkConfig`; `Executor` (`executor/`) applies one message; `CallManager` (`call_manager/`) runs the call stack; each actor invocation gets a `Kernel` (`kernel/`) and a fresh wasmtime instance from `Engine` (`engine/`).
- There are two API boundaries: syscalls between actor code and the kernel (`sdk/src/sys/`, `fvm/src/syscalls/`), and `Externs` between the FVM and the host node (`fvm/src/externs/`: randomness, consensus fault verification, tipset CIDs).
- `fvm/src/{system,init,account,eam}_actor.rs` are state shims the VM reads directly, not actor implementations.
- `shared/` (`fvm_shared`) holds types shared by the VM, actors and hosts. `sdk/` (`fvm_sdk`) is the actor-side syscall binding. `ipld/` holds independently versioned IPLD crates. `testing/` and `tools/` are unpublished.
- `fvm_shared`, `fvm_sdk` and the ipld crates also compile to `wasm32-unknown-unknown` inside actor bundles. Host-only dependencies, std-only features or code-size growth there land in builtin-actors.
- Protocol changes require a Filecoin Improvement Proposal before implementation.

### Before changing or reviewing these, read

- `fvm/src/engine/`, wasmtime or `fvm-wasm-instrument` versions, the accepted WASM feature set, or execution limits: `fvm/src/engine/AGENTS.md`
- Adding or changing a syscall, kernel trait or `sdk/src/sys/`: `fvm/src/syscalls/AGENTS.md`
- `ipld/` crates: `ipld/AGENTS.md`
- Releases and backports: `RELEASE.md`
- Testing ref-fvm changes against builtin-actors: `CONTRIBUTING.md`
- Gas calibration: `testing/calibration/README.md`
- Conformance vectors: `testing/conformance/README.md`

## Execution model

- `Executor::execute_message` takes an `ApplyKind`. Explicit messages are signed chain messages; the sender pays gas. Implicit messages are system calls such as cron; nonce and gas payment are skipped.
- Preflight charges inclusion gas, checks the sender and nonce, and deducts `gas_fee_cap * gas_limit`. A message that fails preflight returns `Ok(ApplyRet)` with a penalty and leaves state unchanged. Once preflight passes, the nonce bump and fee deduction persist even if execution reverts.
- Each send runs in a state-tree transaction, and a non-zero exit code reverts it; exit codes are results, not Rust errors. A `METHOD_SEND` (0) to an existing actor transfers value without running WASM. Sending to an unresolved f1 or f3 address creates an account actor; an unresolved f4 address in the EAM namespace gets a placeholder.
- The executor charges gas outputs (`fvm/src/gas/outputs.rs`): base fee burn on gas used, miner tip on gas limit, and an over-estimation burn.
- The FVM computes the events root but discards the event blocks; hosts persist `ApplyRet.events` themselves.
- Builtin actor type IDs are positions in the manifest, exposed by the `get_builtin_actor_type` syscall, so manifest order is consensus.

## Correctness constraints

- Execution must be deterministic across hosts, platforms and runs: same state root, receipts and gas. Keep wall-clock time, host state, unordered iteration and platform-dependent arithmetic out of consensus paths. Gas timers feed tracing and calibration only; the `gas_calibration` feature isn't for production builds.
- Behaviour and gas change only at a new network version, through a FIP. `price_list_by_network_version` maps each network version to a price list and panics outside the supported range. Never edit a price list that a released network version uses; add a list (usually the previous one with overrides) and map the new version to it. Prices come from benchmarks (`testing/calibration`) and need a FIP to justify a change.
- Actor WASM is metered by instrumentation that `fvm-wasm-instrument` injects at load, so changing the instrumenter or the WASM rules changes gas for every bundle. wasmtime still decides what loads, how it traps and where the stack overflows; see `fvm/src/engine/AGENTS.md`.
- Choose error classes deliberately. Actor-reachable failures are syscall errors (`ErrorNumber`) or exit codes. `ExecutionError::Fatal` means the FVM or host failed: the executor ends the message with `SYS_ASSERTION_FAILED`, charges the full gas limit, and nodes that disagree about a fatal error fork. Missing or undecodable state that the FVM reads for itself is fatal; don't soften it into a syscall error.
- Shipped interfaces are consensus: syscall names and signatures, out-struct layouts, `ErrorNumber` and `ExitCode` values, the order of gas charges inside a syscall, and the CBOR encoding of `fvm_shared` types such as `Message`, `Receipt` and `Address`, which Go hosts mirror.
- `NetworkConfig` and `MachineContext` fields that reach execution are consensus parameters the host sets for every node. `actor_debugging` is one: `debug::enabled` reports it to actors, which branch on it.
- Some syscalls are behind Cargo features, and a host links them only if it turns the feature on. ffi and Forest build every major with `default-features = false`, so ref-fvm's defaults don't apply to them; each must list the features it needs. `verify-signature` is the live case: builtin-actors still calls the `verify_signature` syscall FIP-0079 specified removing, so dropping it breaks current and historical execution. A new feature-gated syscall needs matching ffi and Forest changes.
- Bound resources by the execution limits in `NetworkConfig` (call depth, per-instance and total memory, WASM stack). Inputs from messages are untrusted even when the actor code is trusted.

## Breaking changes

Avoid them. Three live majors are already costly to maintain, and a new major means another copy in every host. A minor release that breaks the Rust API also costs every consumer a bump. When a change is tempted to break behaviour or an interface, gate the new behaviour on a network version inside the current major and keep the old path for earlier versions; prefer additive APIs beside existing ones. A breaking change needs strong justification, stated in the PR.

## Backports and crate coexistence

- `release/v2` and `release/v3` must reproduce history. Behaviour, gas and syscall changes don't land there; an execution bug on those branches is part of the chain. They take dependency and security bumps, toolchain and CI changes, build fixes and small additive APIs that hosts need. A vulnerability reachable only through replay of finalised messages is low severity, since nobody can add new messages to that history.
- Dependency bumps can still change behaviour on any branch. Treat crypto, encoding, WASM and proofs dependencies as consensus dependencies, e.g. a signature library that tightens or relaxes what it accepts.
- ffi and Forest resolve all three majors in one Cargo graph. Shared crates (`fvm_ipld_*`, `cid`, `multihash*`, `wasmtime`, `cranelift-*`) must resolve to one version across them; two copies of `fvm_ipld_blockstore` or `fvm_ipld_encoding` give the host incompatible traits. When master moves a shared crate, check that both release branches can take the same version and that ffi's lockfile resolves. Backport in separate PRs per branch; each branch releases itself.
- The release branches take `fvm_ipld_*` from crates.io; ipld crates are released from master only.

## Build and test

Use the toolchain from `rust-toolchain.toml`, which includes the `wasm32-unknown-unknown` target. Default features enable `opencl`, so build and lint with `--no-default-features` as CI does unless you need proofs GPU support; `make lint` runs clippy with default features.

```sh
cargo fmt --all --check
cargo clippy --all --all-targets --no-default-features -- -D warnings
make license                                  # adds missing license headers; CI fails on any
cargo test --package fvm --no-default-features
cargo test --all --exclude fvm --exclude fvm_conformance_tests --exclude fvm_integration_tests --exclude "*actor"
cargo test --package fvm_integration_tests --package "*actor"   # builds test actors to WASM
```

CI also checks other feature combinations, `cargo deny`, `cargo shear` (unused dependencies) and gas calibration; `.github/workflows/` has the exact jobs. Iterate with the smallest relevant test and report the checks you didn't run.

- Integration tests live in `testing/integration/tests/`, driving actors from `testing/test_actors/actors/`; a new test actor needs an entry in `testing/test_actors/build.rs`.
- Conformance (`git submodule update --init`, then `cargo test --package fvm_conformance_tests`) replays extracted mainnet messages. The corpus covers only network versions 17 and 18, so it checks `release/v2` and `release/v3`; master skips both and runs none. Checking execution from network version 21 on means syncing or re-validating chain through a host (Lotus via ffi, or Forest). For changes that could affect execution, state what you ran.

## Change hygiene

Add tests that would fail for a plausible regression. Report suspected vulnerabilities privately as `SECURITY.md` describes, never in a public issue or PR.

### Comments, docs and commit messages

- Write code comments and reference docs year-zero: describe the code as it is, as if it had always been this way. Changelogs and release notes may compare old and new behaviour. History (old behaviour, the bug, the fix, the issue) goes in the commit message and PR description. In comments, words like "now", "previously", "used to", "no longer", "instead of" and "fixed" signal change narration; rewrite or delete. A link that explains a constraint that still applies stays.
- A comment, in code or tests, earns its place by stating what a maintainer can't read from the code: intent, an invariant, a compatibility or consensus constraint, a surprising cost. Don't restate the code, the test name or the assertion; prefer one precise sentence. Public API docs state the contract: inputs, outputs, errors, bounds.
- Commit messages and PR descriptions explain what changed and why, for a reviewer. Skip summaries of the diff, boilerplate sections and claims you haven't verified.

### Commits, changelogs and versions

Use Conventional Commits form, `<type>(<scope>): <description>`, for commit subjects and PR titles.

A PR that changes a published crate adds an entry under `[Unreleased]` in that crate's `CHANGELOG.md`, prefixed `BREAKING:` when it breaks the Rust API. Versioning of the primary crates isn't strict semver (`RELEASE.md`); note that a wasmtime major is a minor release, because embedders must use the same major. Release steps: `RELEASE.md`.

## Reviewing changes

Read the relevant callers, traits and tests before commenting, and verify assumptions against current code.

Priorities, in order: determinism and network-version gating; gas; syscall ABI and error classes; breaking changes, release-branch compatibility and the shared Cargo graph; resource limits; tests and changelogs. The constraints above define each.

Comment only on a concrete, plausible failure the change introduces: the triggering input or state, the wrong result, its consequence. Skip style preferences, speculative hardening, pre-existing problems and refactors without behavioural benefit. One exception: this repository treats comments that narrate the change or restate the code as defects, so report them (see Comments, docs and commit messages).

## Maintaining agent context

This file and the `AGENTS.md` files it lists are maintained with the code. When a change makes one of them wrong, fix it in the same PR. When a change alters a contract with a neighbouring repository, name the follow-up that repository needs, including its `AGENTS.md`, in the PR description.

Each `AGENTS.md` has a sibling `CLAUDE.md` containing only `@AGENTS.md`, because by default Claude Code skips `AGENTS.md` files whenever a `CLAUDE.md` exists at or above its working directory. Add the pair together; put content in `AGENTS.md`, never in the `CLAUDE.md`.
