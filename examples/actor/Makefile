PROFILE ?= actor
# It lives in the workspace root by default
CARGO_TARGET_DIR ?= ../../target

build:
	cargo build --target=wasm32-unknown-unknown --profile=$(PROFILE)
	cp "$(CARGO_TARGET_DIR)/wasm32-unknown-unknown/$(PROFILE)/fvm_example_actor.wasm" .

decompile: build
	wasm2wat -o fvm_example_actor.wat "$(CARGO_TARGET_DIR)/wasm32-unknown-unknown/$(PROFILE)/fvm_example_actor.wasm"
