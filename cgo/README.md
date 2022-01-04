# CGO FVM Bindings

This package bridges a go-based `Externs` and `Blockstore` with any CGO-based module. To use it:

1. In your rust application, import the rust bindings (`./rust`).
2. In your go application, register an externs or a blockstore to get back a handle.
3. Pass the handle into your cgo library.
4. Use that handle to operate on the go-based externs/blockstore.

Take a look at the [../example](../example) directory for how all this fits together.
