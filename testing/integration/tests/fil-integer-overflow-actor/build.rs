// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
fn main() {
    use substrate_wasm_builder::WasmBuilder;
    WasmBuilder::new()
        .with_current_project()
        .import_memory()
        .append_to_rust_flags("-Ctarget-feature=+crt-static")
        .append_to_rust_flags("-Cpanic=abort")
        .append_to_rust_flags("-Clto=true")
        .append_to_rust_flags("-Copt-level=z")
        .build()
}
