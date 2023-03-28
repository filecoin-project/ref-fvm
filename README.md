# Reference Filecoin VM implementation (v3; dev)

[![Continuous integration](https://github.com/filecoin-project/ref-fvm/actions/workflows/ci.yml/badge.svg)](https://github.com/filecoin-project/ref-fvm/actions/workflows/ci.yml)

This repository contains the reference implementation of the Filecoin VM ([specs](https://github.com/filecoin-project/fvm-project)). It is written in Rust, and intended to be integrated via FFI into non-Rust clients (e.g. Lotus, Fuhon), or directly into Rust clients (e.g. Forest). FFI bindings for Go are provided in-repo, and developers are encouraged to contribute bindings for other languages.


See the [Project Website](https://fvm.filecoin.io/) for details.

## Build requirements

* Install [rustup](https://rustup.rs/).

## Build instructions

```sh
$ git clone https://github.com/filecoin-project/ref-fvm.git
$ cd fvm
$ rustup target add wasm32-unknown-unknown
$ make
```

## Code structure

Here's what you'll find in each directory:

- `/fvm`
  - The core of the Filecoin Virtual Machine. The key concepts are:
    - `Machine`: an instantiation of the machine, anchored at a specific state root and epoch, ready to intake messages to be applied.
    - `Executor`: an object to execute messages on a `Machine`.
    - `CallManager`: tracks and manages the call stack for a given message.
    - Invocation container (conceptual layer, not explicitly appearing in code): the WASM instance + sandbox under which a given actor in the call stack runs.
    - `Kernel`: the environment attached to an invocation container for external interactions.
  - There are two API boundaries in the system:
    1. the boundary between the actor code and the Kernel, which is traversed by invoking `Syscalls`.
    2. the boundary between the FVM and the host node, represented by `Externs`.
  - Some parts of the FVM are based on the [Forest](https://github.com/ChainSafe/forest) implementation.
- `/sdk`
  - Reference SDK implementation to write Filecoin native actors, used by the canonical built-in actors through the Actors FVM Runtime shim.
  - User-defined FVM actors written in Rust can also use this SDK, although it is currently quite rough around the edges. In the next weeks, we expect to sweeten it for improved developer experience.
  - Alternative SDKs will emerge in the community. We also expect community teams to develop SDKs in other WASM-compilable languages such as Swift, Kotlin (using Kotlin Native), and even Go (via the TinyGo compiler).
- `/shared`
  - A crate of core types and primitives shared between the FVM and the SDK.
- `/ipld`
  - IPLD libraries. Some of which are based on, and adapted from, the [Forest](https://github.com/ChainSafe/forest) implementation.
- `/testing/conformance`
  - Contains the test vector runner, as well as benchmarking utilities on top of it.
  - The conformance test runner feeds the test vector corpus located at https://github.com/filecoin-project/fvm-test-vectors into ref-fvm, in order to validate spec conformance.
  - The benchmarking utilities use the `criterion` Rust library to measure the performance and overhead of ref-fvm across various facets.
  - See the [instructions](./testing/conformance/README.md#instructions) about how to run the tests and the benchmarks.
  - Disclaimers
    - Benchmarks are currently very slow to run, setup and teardown. This is due to using default WASM cache, and will be fixed soon.

## License

Dual-licensed: [MIT](./LICENSE-MIT), [Apache Software License v2](./LICENSE-APACHE), by way of the
[Permissive License Stack](https://protocol.ai/blog/announcing-the-permissive-license-stack/).

---

actors and vm forked from [ChainSafe/forest](https://github.com/ChainSafe/forest)
commit: [`73e8f95a108902c6bef44ee359a8478663844e5b`](https://github.com/ChainSafe/forest/commit/73e8f95a108902c6bef44ee359a8478663844e5b)
