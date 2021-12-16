# Reference Filecoin VM implementation (work-in-progress 🚧)

> 🚧⚠️🐉🐍🐛 This repo does not contain a working implementation yet. It is under **HEAVY** construction. Note the vast amount of TODOs and open issues. Pace of development is very high, and the code changes substantially everyday. Keep visiting!

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

- `/fvm` 🚧: contains the core of the Filecoin Virtual Machine. The key concepts are the `Machine` (an instantiation of the machine, anchored at a specific state root and epoch, ready to intake messages to be applied), the `CallManager` (tracks and manages the call stack for a given message application), the invocation container (which does not have a corresponding struct, but is a logical layer; it is the WASM instance + sandbox under which a given actor in the call stack runs), and the `Kernel` (the environment attached to an invocation container). There are two API boundaries in the system: the boundary between the actor code and the Kernel, which is traversed by invoking `Syscalls`, and the boundary between the FVM and the host node, represented by `Externs`. Some parts of the FVM are based on the [Forest](https://github.com/ChainSafe/forest) implementation.
- `/sdk` 🚧: the SDK used by FVM actors, written in Rust and serving as a reference implementation. Used by the canonical built-in actors. User-defined FVM actors written in Rust can also use this SDK, although alternatives may emerge in the community. Similarly, we expect community teams to develop SDKs in other WASM-compilable languages such as Swift, Kotlin (using Kotlin Native), and even Go (via the TinyGo compiler).
- `/actors` 🚧: the canonical built-in actors, adapted to be deployed _inside_ the FVM, with trimmed down dependencies, and their Runtime bridging to the FVM SDK. Largely based off the [Forest](https://github.com/ChainSafe/forest) implementation.
- `/shared` 🚧: a crate of core types and primitives shared between the FVM and the SDK.
- `/cgo` 🚧: components serving the Cgo boundary between Go and Rust. Concretely, today it contains a blockstore adapter used to inject a blockstore owned by Go code, to the FVM built in Rust.
- `/lib` and `/ipld` 🚧: various libraries, mostly related to IPLD data processing. Some of them are based off, and adapted, from the [Forest](https://github.com/ChainSafe/forest) implementation.
- `/examples` 🚧: a directory eventually containing actor examples.
- `/_forest` 🚧: a subtree containing some relevant components of Forest, conveniently colocated with the FVM, to use as a quick reference during development. This subtree is not linked to the build.


## License

Dual-licensed: [MIT](./LICENSE-MIT), [Apache Software License v2](./LICENSE-APACHE), by way of the
[Permissive License Stack](https://protocol.ai/blog/announcing-the-permissive-license-stack/).

---

actors and vm forked from [ChainSafe/forest](https://github.com/ChainSafe/forest)
commit: [`73e8f95a108902c6bef44ee359a8478663844e5b`](https://github.com/ChainSafe/forest/commit/73e8f95a108902c6bef44ee359a8478663844e5b)
