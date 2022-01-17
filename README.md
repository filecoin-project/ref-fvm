# Reference Filecoin VM implementation (pre-alpha)

[![Continuous integration](https://github.com/filecoin-project/fvm/actions/workflows/ci.yml/badge.svg)](https://github.com/filecoin-project/fvm/actions/workflows/ci.yml)

This repository contains the reference implementation of the Filecoin VM ([specs](https://github.com/filecoin-project/fvm-project)). It is written in Rust, and intended to be integrated via FFI into non-Rust clients (e.g. Lotus, Fuhon), or directly into Rust clients (e.g. Forest). FFI bindings for Go are provided in-repo, and developers are encouraged to contribute bindings for other languages.

## Build requirements

* The current MSRV (Minimum Supported Rust Version) is 1.59 (nightly). A working version is tracked in `rust-toolchain` (this is picked up by `rustup` automatically).
* Install [rustup](https://rustup.rs/).

## Build instructions

```sh
$ git clone https://github.com/filecoin-project/fvm.git
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
- `/actors`
  - the canonical built-in actors, adapted to be deployed _inside_ the FVM, with trimmed down dependencies, and their Runtime bridging to the FVM SDK. Largely based off the [Forest](https://github.com/ChainSafe/forest) implementation.
- `/shared`
  - A crate of core types and primitives shared between the FVM and the SDK.
- `/cgo`
  - Components serving the Cgo boundary between Go and Rust. Concretely, today it contains a blockstore adapter used to inject a blockstore owned by Go code, to the FVM built in Rust.
- `/ipld`
  - IPLD libraries. Some of which are based on, and adapted from, the [Forest](https://github.com/ChainSafe/forest) implementation.
- `/examples`
  - A directory eventually containing actor examples.

## Maturity roadmap

### v0: FVM running built-in actors

- Alpha:
  - Declared when: all test vectors passing, integrated into Lotus via FFI.
  - Focus: theoretical correctness.
  - Estimated: end of January '22.
- Beta: 
  - Declared when: all of the above + syncing mainnet consistently, keeping up with chain consistently.
  - Focus: production-readiness, performance, live consensus correctness.
  - Estimated: late February '22.
- RC:
  - Declared when: all of the above + integrated into a second client (likely Forest), successfully syncing mainnet on all.
  - Focus: pre-mainnet preparations.
  - Estimated: March '22.
- Final:
  - Declared when: FVM v0 is securing mainnet, i.e. when M1 from the [FVM milestone roadmap](https://filecoin.io/blog/posts/introducing-the-filecoin-virtual-machine/) is reached.
  - Estimated: end of March '22.

### v1: Fully-programmable FVM (with EVM foreign runtime support)

- Alpha:
  - Declared when: all functionality implemented, 70%+ test coverage, integrated into Lotus via FFI.
  - Focus: feature completeness.
  - Estimated: end of April '22.
- Beta:
  - Declared when: testnets deployed (user and automatic), running successfully for 1 week.
  - Focus: testing and hardening.
  - Estimated: May '22.
- RC:
  - Declared when: code audited; network-upgrade releases ready for all Filecoin clients.
  - Focus: pre-mainnet preparations.
  - Estimated: June '22.
- Final:
  - Declared when: FVM v1 is operating mainnet, i.e. when M2 from the [FVM milestone roadmap](https://filecoin.io/blog/posts/introducing-the-filecoin-virtual-machine/) is reached.
  - Estimated: end of June '22.

## License

Dual-licensed: [MIT](./LICENSE-MIT), [Apache Software License v2](./LICENSE-APACHE), by way of the
[Permissive License Stack](https://protocol.ai/blog/announcing-the-permissive-license-stack/).

---

actors and vm forked from [ChainSafe/forest](https://github.com/ChainSafe/forest)
commit: [`73e8f95a108902c6bef44ee359a8478663844e5b`](https://github.com/ChainSafe/forest/commit/73e8f95a108902c6bef44ee359a8478663844e5b)
