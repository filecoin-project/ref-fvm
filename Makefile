RUSTFLAGS="-C target-feature=+crt-static"

all: build examples
.PHONY: all

build:
	cargo build --features builtin_actors
.PHONY: build

test-examples: test-example-token
.PHONY: test-examples

#examples: example-actor example-fvm example-blockstore-cgo
# take the fvm examples out of the build tree; the examples will be superseded
# by tests
examples: example-actor example-blockstore-cgo
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

example-token:
	cargo build --package fvm_token_actor
.PHONY: example-token

test-example-token:
	cargo test --package fvm_token_actor
.PHONY: test-example-token

clean:
	cargo clean

lint: clean
	cargo fmt --all
	cargo clippy --all -- -D warnings -A clippy::upper_case_acronyms
