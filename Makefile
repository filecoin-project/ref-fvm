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
