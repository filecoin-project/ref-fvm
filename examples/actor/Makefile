PROFILE ?= actor

build:
	cargo build --target=wasm32-unknown-unknown --profile=$(PROFILE)

decompile: build
	wasm2wat -o fvm_example_actor.wat fvm_example_actor.wasm
