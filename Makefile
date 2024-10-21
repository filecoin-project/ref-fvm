all: build
.PHONY: all

build:
	cargo build
.PHONY: build

clean:
	cargo clean
.PHONY: clean

lint:
	cargo fmt --all --check
	cargo clippy --all --all-targets -- -D warnings
.PHONY: lint

license:
	cargo run --bin check-license -- .
.PHONY: license
