build:
	cargo build --target=wasm32-unknown-unknown --release
	cp target/wasm32-unknown-unknown/release/fvm_runtime.wasm .

decompile: build
	wasm2wat -o fvm_runtime.wat fvm_runtime.wasm
