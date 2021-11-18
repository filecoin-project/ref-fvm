use blockstore::{Blockstore, MemoryBlockstore};
use cid::Cid;
use fvm;
use multihash::{Code as MhCode, MultihashDigest};
use wasmtime::{Config, Engine, Module, Store};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let module_wasm = include_bytes!("../fvm_example_actor.wasm");
    let mut engine = Engine::new(&Config::default())?;
    let module = Module::new(&engine, module_wasm)?;
    let config = fvm::Config { max_pages: 10 };
    let bs = MemoryBlockstore::default();
    let root_block = b"test root block";
    let root_cid = Cid::new_v1(0x55, MhCode::Sha2_256.digest(root_block));
    bs.put(&root_cid, root_block)?;
    let runtime = fvm::DefaultRuntime::new(config, bs, root_cid);
    let linker = fvm::environment(&mut engine)?;
    let mut store = Store::new(&engine, runtime);
    let instance = linker.instantiate(&mut store, &module)?;

    let add_one = instance
        .get_export(&mut store, "invoke")
        .and_then(|exp| exp.into_func())
        .ok_or_else(|| "invoke missing")?;
    add_one.call(&mut store, &[], &mut [])?;
    Ok(())
}
