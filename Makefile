RUSTFLAGS="-C target-feature=+crt-static"

all: build
.PHONY: all

build:
	cargo build
.PHONY: build

clean:
	cargo clean

lint: license clean
	cargo fmt --all
	cargo clippy --all -- -D warnings -A clippy::upper_case_acronyms

license:
	./scripts/add_license.sh
