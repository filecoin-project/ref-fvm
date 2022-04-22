RUSTFLAGS='-Ctarget-feature=+bulk-memory -Ctarget-feature=+crt-static' cargo build --target wasm32-unknown-unknown --profile actor "$@"
cp ../../target/wasm32-unknown-unknown/actor/fuel_samples.wasm .
wasm-opt -O3 --enable-bulk-memory -o samples.wasm fuel_samples.wasm
cp samples.wasm fuel_samples.wasm ../fuel-est
