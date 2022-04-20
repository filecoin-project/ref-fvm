RUSTFLAGS='-Ctarget-feature=+bulk-memory -Ctarget-feature=+crt-static' cargo build --target wasm32-unknown-unknown --profile actor "$@"
