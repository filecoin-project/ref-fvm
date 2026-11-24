# Syscalls (fvm/src/syscalls/, fvm/src/kernel/, sdk/src/sys/)

Syscalls are the only interface actor code has to the VM, and shipped bundles import them by name. The ABI is specified in [fvm-specs](https://github.com/filecoin-project/fvm-specs/blob/main/08-syscalls.md); the SDK error tables in `sdk/src/sys/` are not authoritative, so check the kernel for the errors a syscall returns.

## Shape of a syscall

- `sdk/src/sys/<module>.rs` declares it inside `fvm_syscalls! { module = "<module>"; ... }`. The Rust function name is the WASM import name. A `Result<T>` return with a non-unit `T` adds a leading out-pointer, and a non-zero return becomes an `ErrorNumber`.
- `fvm/src/syscalls/mod.rs` binds it with `link_syscall("<module>", "<name>", host_fn)`. The strings must match the SDK declaration; a bundle importing a syscall the host didn't link fails at instantiation as `Abort::Fatal`.
- The host function in `fvm/src/syscalls/<module>.rs` takes `Context<'_, impl <Sub>Ops>` and integer arguments, checks memory bounds, and calls the kernel.
- Kernel functionality is split into sub-traits in `fvm/src/kernel/mod.rs` (`NetworkOps`, `CryptoOps`, ...), implemented by `DefaultKernel` (`kernel/default.rs`). Filecoin-specific syscalls (proofs, circulating supply, consensus faults) belong to `FilecoinKernel` (`kernel/filecoin.rs`) and are linked only for `DefaultFilecoinKernel`.

## Adding a syscall

1. Declare it in `sdk/src/sys/<module>.rs`, with its error table, and add the safe wrapper in `sdk/src/<module>.rs`.
2. If it returns a struct, add it to `shared/src/sys/out.rs`. Out-structs are `#[repr(packed, C)]`, `Copy`, and free of pointers and floats; `assert_syscall_safe!` checks them.
3. Write the host function and link it in `fvm/src/syscalls/mod.rs`.
4. Add the method to the right kernel sub-trait and implement it in `DefaultKernel`, or in `FilecoinKernel` for Filecoin-specific calls.
5. Price it in `fvm/src/gas/price_list.rs`: a `PriceList` field, a value in each price list the syscall can run under, and an `on_<name>` method. Charge before the computation or allocation the charge pays for. A new syscall changes execution, so it ships at a new network version.
6. A new sub-trait needs `#[delegate]` in the three delegating kernels (`kernel/filecoin.rs`, `testing/conformance/src/vm.rs`, `testing/integration/src/custom_kernel/mod.rs`) and a bound in the `SyscallHandler` impls. Sub-trait methods reach those kernels through delegation; `FilecoinKernel` methods are written by hand in the conformance and integration kernels.
7. Exercise it from a test actor (`testing/test_actors/actors/fil-syscall-actor`) with an integration test, and record it in the `fvm` and `sdk` changelogs.

builtin-actors adopts it only by depending on a released `fvm_sdk`.

## Errors

- `ExecutionError` is `OutOfGas`, `Syscall(SyscallError)` or `Fatal`. Return actor-reachable failures with `syscall_error!` or `or_error`/`or_illegal_argument`; use `or_fatal` only for FVM or host failure. Extern failures are fatal.
- Treat `ErrorNumber` as closed despite `#[non_exhaustive]`. The SDK `expect`s a known code, so adding a variant makes actors built with older SDKs panic.
- `vm::exit` aborts the actor and never returns; a system exit code passed to it becomes `SYS_ILLEGAL_EXIT_CODE`.
- Running out of gas in the syscall prologue stays `OutOfGas`; a syscall error from that charge escalates to fatal. A bad return pointer is an `IllegalArgument` after execution and syscall gas are charged.

## Compatibility rules

- Never rename a shipped syscall, change its module, arity or types, or change an out-struct layout. Removing a syscall breaks every bundle that imports it, including historical ones.
- Every syscall charges `wasm_exec` and `OnSyscall` first. `batch_verify_seals` deliberately charges nothing more: the power actor calls it from cron, and providers prepay at pre-commit. Inside the kernel, some syscalls validate then charge and others charge then validate; the order is consensus once shipped, so don't normalise it.
- Gating is by Cargo feature, not network version: `verify-signature`, `upgrade-actor` and `m2-native` (`fvm/Cargo.toml`, `sdk/Cargo.toml`). Hosts enable features explicitly; see the root `AGENTS.md` on host features.
- `debug::log` and `debug::store_artifact` are always linked and do nothing unless `actor_debugging` is set. `debug::enabled` reports the flag, and actors branch on it, so only local tests or a network-wide parameter enable it.
