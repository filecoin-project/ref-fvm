CARGO_TARGET_DIR ?= target
build:
	cargo build --target=wasm32-unknown-unknown --release
	cp "$(CARGO_TARGET_DIR)/wasm32-unknown-unknown/release/fvm_example_actor.wasm" .

decompile: build
	wasm2wat -o fvm_example_actor.wat fvm_example_actor.wasm
