# WASM engine (fvm/src/engine/)

wasmtime executes every actor on every node. A wasmtime change can alter consensus without any ref-fvm code changing, so it is never a routine dependency update, whatever dependabot or CI says. Green CI shows the tests pass, not that accepted code, trap outcomes and stopping points match the previous version.

## Loading and metering

`Engine::load_raw` validates the module, injects the stack limiter (`fvm_wasm_instrument::stack_limiter`), then injects gas metering (`gas_metering::inject`) priced by `WasmGasPrices` from the network version's price list. Instrumented code decrements an imported `gas.gas_counter` global; the FVM syncs it around every syscall and invocation (`fvm/src/syscalls/mod.rs`). wasmtime fuel and epoch interruption stay off. Modules are cached by code CID and instantiated fresh for every call; `InstancePool` bounds pooling-allocator slots rather than reusing instances.

Gas is fixed by the bundle bytes and the instrumenter. Rebuilding a bundle or changing `fvm-wasm-instrument` changes gas; a wasmtime bump changes native speed, not gas charged.

## Consensus-critical properties

- **Accepted WASM.** `wasmtime_config` denies every feature, then allows `FVM_WASM_FEATURES`. `fvm-wasm-instrument` meters only that set, so anything else would run unmetered. Changing the set needs a FIP. The explicit `wasm_*(false)` calls are redundant with the mask on purpose: each documents a rejection and its reason. Keep both, and add a call with a reason when wasmtime adds a proposal, since wasmtime enables proposals by default as they mature.
- **Reference types stay enabled.** builtin-actors bundles from v17 encode the `call_indirect` table index as a padded LEB that only the reference-types proposal permits, so removing `REFERENCE_TYPES` fails validation of those bundles. `padded_call_indirect_table_index_is_accepted` covers both encodings.
- **wasmtime Cargo features.** Keep `default-features = false`. Features such as `gc`, `threads`, `component-model`, `async`, `cache` and `wasi` turn on code paths and proposals; walk what one enables before adding it.
- **Trap mapping.** `fvm/src/syscalls/error.rs` maps specific `Trap` variants to `SYS_ILLEGAL_INSTRUCTION`; any other trap is `Abort::Fatal`, which fails the message. `Trap` is `#[non_exhaustive]` and grows between releases, and the `Abort` and `Trap` downcasts from `wasmtime::Error` must keep working.
- **Stopping point.** The instrumented stack limit must fire before wasmtime's native `max_wasm_stack`, or the same recursion ends differently. Inlining, register allocation and frame layout eat that headroom per architecture, which is why those settings are pinned.
- **Engine settings are contract**, whether set explicitly (memory reservation and guard size, signals-based traps) or left at the default (lazy table init). `effective_engine_config_is_fixed` reads the consensus-relevant settings back from the built `Engine`, so a moved default fails CI. If it fails after a bump, investigate; don't update the assertion to match. `accepted_wasm_features_are_fixed` does the same for the feature set.
- **macOS traps.** Keep `macos_use_mach_ports(false)` on every branch. With Mach ports, traps hosted inside Lotus's Go runtime were intermittently missed; FVM uses signal handlers on every platform, unlike wasmtime's documented macOS default.

Limits come from `NetworkConfig` (`fvm/src/machine/mod.rs`): call depth, per-instance memory through the pooling allocator, total memory across the call stack through `DefaultMemoryLimiter`, and WASM stack through instrumentation.

## Upgrading wasmtime

Use LTS majors only (every 12th major), which get two years of security backports. `deny.toml` bans the non-LTS ranges for `wasmtime` and `wasmtime-environ` on all three branches, and `.github/dependabot.yml` on master ignores them and groups both crates; adopting the next LTS extends both lists everywhere in one change.

Before landing any wasmtime change, including a patch bump:

- Read every release note between the versions, patch releases too, and classify each item: default change, new knob or feature, trap change, API break, Cranelift codegen or determinism fix, security advisory (reachable with our config or not), MSRV. Record the walk where reviewers can see it.
- Diff `Config::features()`, `Tunables` defaults, the `Trap` enum and the `[features]` table in the published crates, not only the notes.
- Move master, `release/v3` and `release/v2` together, to the same exact versions of every `wasmtime-*`, `cranelift-*` and `pulley-*` crate, since hosts resolve all three majors in one Cargo graph (root `AGENTS.md`, Backports and crate coexistence). Branch lockfiles differ, so use `cargo update -p wasmtime --precise <version>` per branch rather than cherry-picking lockfile commits.
- Move `rust-toolchain.toml` on all three branches and in ffi when wasmtime raises its MSRV; new toolchains bring new lints under CI's `-Dwarnings`.
- Run conformance on `release/v2` and `release/v3`, and check that each branch still loads every bundle in its range.
- Run under a host, not only `cargo test`. Lotus hosts wasmtime inside the Go runtime through cgo, and signal-based trap handling interacts with Go's signals and stacks. Cover linux/amd64, linux/arm64 and darwin/arm64 with a trap-heavy workload and a chain sync or re-validation.
- A wasmtime major is a minor release on every branch, since embedders must use the same major.

Dependabot can't do this review. Security updates open only against the default branch, so after one lands on master the release branches are behind until someone bumps them by hand. An ignored range filters dependabot's target version, not cargo's resolution; close release-branch PRs that pick a banned version.
