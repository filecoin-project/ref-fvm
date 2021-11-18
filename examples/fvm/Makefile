build:
	cp "../../target/wasm32-unknown-unknown/actor/fvm_example_actor.wasm" .
	cargo build

decompile: build
	wasm2wat -o fvm_example_actor.wat fvm_example_actor.wasm
