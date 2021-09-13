# FVM Runtime Interfaces

This package contains the actor-side runtime interfaces for the FVM. The FFI interface (to the
runtime itself) is defined in the `sys` package.

## Design Notes

### Returning/Aborting

TODO

### Syscalls

The Filecoin VM exposes ["syscalls"][syscall] via special functions on the runtime. To avoid expanding the runtime interface too much, I'm hoping to treat these methods as normal actor methods (ish) rather than special runtime calls.

[syscall]: https://github.com/filecoin-project/specs-actors/blob/58cb5de23d1f05bef3639e4412309b50d78f1c2e/actors/runtime/runtime.go#L173

### IPLD Interface

TODO
