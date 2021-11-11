CARGO_TARGET_DIR ?= target
PROFILE ?= actor

build:
	cargo build --target=wasm32-unknown-unknown --profile=$(PROFILE)
	cp "$(CARGO_TARGET_DIR)/wasm32-unknown-unknown/$(PROFILE)/fvm_example_actor.wasm" .

decompile: build
	wasm2wat -o fvm_example_actor.wat fvm_example_actor.wasm
