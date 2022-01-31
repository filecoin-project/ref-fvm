RUSTFLAGS="-C target-feature=+crt-static"

all: build examples
.PHONY: all

build:
	cargo build --features builtin_actors
.PHONY: build

examples: example-actor
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

clean:
	cargo clean

lint: clean
	cargo fmt --all
	cargo clippy --all -- -D warnings -A clippy::upper_case_acronyms
