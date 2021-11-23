RUSTFLAGS="-C target-feature=+crt-static"

all: build examples
.PHONY: all

build:
	cargo build
.PHONY: build

examples: example-actor example-fvm example-blockstore-cgo
.PHONY: examples

example-actor:
	$(MAKE) -C ./examples/actor build
.PHONY: example-actor

example-fvm: example-actor
	$(MAKE) -C ./examples/fvm build
.PHONY: example-fvm

example-blockstore-cgo:
	$(MAKE) -C ./examples/blockstore-cgo
.PHONY: example-blockstore-cgo