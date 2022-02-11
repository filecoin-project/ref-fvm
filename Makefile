RUSTFLAGS="-C target-feature=+crt-static"

all: build examples
.PHONY: all

build:
	cargo build
.PHONY: build

examples: example-actor
.PHONY: examples

example-actor:
	$(MAKE) -C ./examples/actor build
.PHONY: example-actor

clean:
	cargo clean

lint: clean
	cargo fmt --all
	cargo clippy --all -- -D warnings -A clippy::upper_case_acronyms
