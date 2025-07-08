all: build
.PHONY: all

build:
	cargo build
.PHONY: build

clean:
	cargo clean

lint:
	cargo fmt --all --check
	cargo clippy --all --all-targets -- -D warnings

license:
	./scripts/add_license.sh

doctest:
	cargo test --all --exclude fvm --exclude fvm_conformance_tests --exclude fvm_integration_tests --exclude "*actor" --doc
