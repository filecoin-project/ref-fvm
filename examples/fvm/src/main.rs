use fvm;
use wasmer::{Instance, Module, Store};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let module_wasm = include_bytes!("../fvm_example_actor.wasm");
    let store = Store::default();
    let module = Module::new(&store, module_wasm)?;
    let import_object = fvm::environment(&store);
    let instance = Instance::new(&module, &import_object)?;

    let add_one = instance.exports.get_function("invoke")?;
    add_one.call(&[])?;

    Ok(())
}
