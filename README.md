# Reference Filecoin VM implementation

> Specs at: https://github.com/filecoin-project/fvm-project

## Code layout

```
 /
 |__ actor
 |      # built-in actors forked from ChainSafe/forest, ported to the FVM
 |      # using the FVM SDK, and compiled to WASM bytecode. Each actor produces
 |      # a separate WASM bundle.
 |
 |__ {blockchain,encoding,ipld,types,utils}
 |      # dependencies inherited from ChainSafe/forest when forking actors.
 |      # may get pruned and/or refactored.
 |
 |__ cgo/blockstore
 |    | # shim to inject an external blockstore owned by Go code into the FVM.
 |    |
 |    |_ *.go
 |    |    # Go side of the adapter (adapter to make a Go blockstore fulfill the FFI contract).
 |    |
 |    |_ example
 |         # A full example injecting a blockstore from Go, and writing and reading to it from Rust.
 |
 |__ fvm
 |     # the reference implementation of the FVM.
 |
 |__ lib/blockstore
 |     # the blockstore trait as required by the FVM + implementations,
       # including the Cgo implementation which expects a linked and initialized
       # cgo/blockstore library. 
 |
 |__ sdk
 |     # library imported by actors written in Rust targeting the FVM.
 |
 |__ vm
       # the Forest VM.
 ```

## License

Dual-licensed: [MIT](./LICENSE-MIT), [Apache Software License v2](./LICENSE-APACHE), by way of the
[Permissive License Stack](https://protocol.ai/blog/announcing-the-permissive-license-stack/).

---

actors and vm forked from [ChainSafe/forest](https://github.com/ChainSafe/forest)
commit: [`73e8f95a108902c6bef44ee359a8478663844e5b`](https://github.com/ChainSafe/forest/commit/73e8f95a108902c6bef44ee359a8478663844e5b)
