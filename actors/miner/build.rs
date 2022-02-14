fn main() {
    use wasm_builder::WasmBuilder;
    WasmBuilder::new()
        .with_current_project()
        .append_to_rust_flags("-Ctarget-feature=+crt-static")
        .append_to_rust_flags("-Cpanic=abort")
        .append_to_rust_flags("-Coverflow-checks=yes")
        .append_to_rust_flags("-Clto=thin")
        .append_to_rust_flags("-Copt-level=z")
        .build()
}
