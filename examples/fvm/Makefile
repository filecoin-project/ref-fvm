CARGO_TARGET_DIR ?= ../../target

build-actor:
	$(MAKE) -C ../actor build
	cp "$(CARGO_TARGET_DIR)/wasm32-unknown-unknown/actor/fvm_example_actor.wasm" .

build: build-actor
	cargo build

run: build-actor
	RUST_BACKTRACE=1 cargo run
