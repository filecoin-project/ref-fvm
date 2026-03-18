all: build

build:
	cargo build

clean:
	cargo clean

lint:
	cargo fmt --all --check
	cargo clippy --all --all-targets -- -D warnings

license:
	./scripts/add_license.sh

doctest:
        # We disable test packages, given they are unlikely to contain any doctests and would double the compilation duration.
	cargo test --all --exclude fvm_conformance_tests --exclude fvm_integration_tests --exclude "*actor" --doc

doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --all --no-default-features --exclude fvm_conformance_tests --exclude fvm_integration_tests --exclude "*actor"

.PHONY: all build clean lint license doctest doc
