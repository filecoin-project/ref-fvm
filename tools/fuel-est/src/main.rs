use std::cmp::Ordering;
use std::time;

use anyhow::{anyhow, bail, Result};
use fvm::machine::default_wasmtime_config;
use wasmtime::{Engine, Linker, Module, Store, TypedFunc};
fn main() {
    let res = main1();
    println!("{:?}", res);
}

fn main1() -> Result<()> {
    let cfg = default_wasmtime_config();
    let engine = Engine::new(&cfg)?;
    let module = Module::from_file(&engine, "samples.wasm")?;

    let linker = link(&engine)?;

    let mut store = Store::new(&engine, ());
    store.add_fuel(10000000000000000)?;
    let instance = linker.instantiate(&mut store, &module)?;
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| anyhow!("no such memory"))?;
    let list = instance.get_typed_func::<(), i32, _>(&mut store, "list")?;

    let stroff = list.call(&mut store, ())? as usize;

    let strstart = memory
        .data(&store)
        .get(stroff..)
        .ok_or_else(|| anyhow!("no memory at offset"))?;
    let modules_str = std::str::from_utf8(
        strstart
            .split(|x| *x == 0)
            .next()
            .ok_or_else(|| anyhow!("no null byte"))?,
    )?;
    let modules: Vec<_> = modules_str.split('\n').collect();

    for (i, m) in modules.iter().enumerate() {
        let (init, invoke, mut store) = get_invoke(&engine, &module)?;
        const INIT_FUEL: u64 = 1_000_000_000;

        store.add_fuel(INIT_FUEL)?;
        init.call(&mut store, ())?;
        let fuel_consumed = store.fuel_consumed().expect("fuel is enabled");
        match fuel_consumed.cmp(&INIT_FUEL) {
            Ordering::Greater => {store.add_fuel(fuel_consumed - INIT_FUEL)?;}
            Ordering::Less => {store.consume_fuel(INIT_FUEL - fuel_consumed)?;}
            _ => { }
        };


        const FUEL: u64 = 10_000_000_000;
        store.add_fuel(FUEL)?;
        println!("executing {}: {}", i, m);

        let start = time::Instant::now();

        let invoke_res = invoke.call(&mut store, i as u32);

        let elapsed = start.elapsed();
        if store.fuel_consumed().unwrap_or_default() > FUEL {
            if invoke_res.is_ok() {
                bail!("task failed successfully")
            }
        } else {
            invoke_res?;
        }

        println!(
            "{} ns/fuel, took {:3}",
            (elapsed.as_nanos() as f64) / (FUEL as f64),
            elapsed.as_secs_f64()
        );
    }

    Ok(())
}

fn link<T>(engine: &Engine) -> Result<Linker<T>> {
    let mut linker = Linker::new(engine);
    linker.func_wrap("env", "black_box1", |_: i32| {})?;
    linker.func_wrap("env", "black_box2", |ptr: i32| ptr)?;

    Ok(linker)
}

#[allow(clippy::type_complexity)]
fn get_invoke(
    engine: &Engine,
    module: &Module,
) -> Result<(TypedFunc<(), ()>, TypedFunc<u32, ()>, Store<()>)> {
    let linker = link(engine)?;
    let mut store = Store::new(engine, ());
    let instance = linker.instantiate(&mut store, module)?;
    let invoke = instance.get_typed_func::<u32, (), _>(&mut store, "invoke")?;
    let init = instance.get_typed_func::<(), (), _>(&mut store, "init")?;

    Ok((init, invoke, store))
}
