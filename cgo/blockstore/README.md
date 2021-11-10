# CGO Blockstore

This package bridges a go-based blockstore with any CGO-based module. To use it:

1. In your cgo library, import the shim implementation (e.g., `./rust`).
2. In your go application, register a blockstore to get back a handle.
3. Pass the handle into your cgo library.
4. Use that handle to operate on the go-based blockstore.

Take a look at the [./example](./example) directory for how all this fits together.
