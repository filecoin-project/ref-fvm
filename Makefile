RUSTFLAGS="-C target-feature=+crt-static"

all: build examples
.PHONY: all

build:
	cargo build
.PHONY: build

examples: example-actor example-fvm example-blockstore
.PHONY: examples

example-actor:
	$(MAKE) -C ./examples/actor build
.PHONY: example-actor

example-fvm: example-actor
	$(MAKE) -C ./examples/fvm build
.PHONY: example-fvm

example-blockstore:
	$(MAKE) -C ./examples/blockstore
.PHONY: example-blockstore