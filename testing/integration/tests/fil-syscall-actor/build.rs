fn main() {
    use wasm_builder::WasmBuilder;
    WasmBuilder::new()
        .with_current_project()
        .import_memory()
        .append_to_rust_flags("-Ctarget-feature=+crt-static")
        .append_to_rust_flags("-Cpanic=abort")
        .append_to_rust_flags("-Coverflow-checks=true")
        .append_to_rust_flags("-Clto=true")
        .append_to_rust_flags("-Copt-level=z")
        .append_to_rust_flags("-Zinstrument-coverage")
        .append_to_rust_flags("-Zno-profiler-runtime")
        .append_to_rust_flags("-Clink-dead-code")
        .build()
}
