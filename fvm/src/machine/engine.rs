use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_wasm_instrument::gas_metering::GAS_COUNTER_NAME;
use wasmtime::{Global, GlobalType, Linker, Module, Mutability, Val, ValType};

use crate::machine::MachineContext;
use crate::syscalls::{bind_syscalls, InvocationData};
use crate::Kernel;

/// A caching wasmtime engine.
#[derive(Clone)]
pub struct Engine(Arc<EngineInner>);

pub fn default_wasmtime_config() -> wasmtime::Config {
    let mut c = wasmtime::Config::default();

    // wasmtime default: false
    c.wasm_threads(false);

    // wasmtime default: true
    c.wasm_simd(false);

    // wasmtime default: false
    c.wasm_multi_memory(false);

    // wasmtime default: false
    c.wasm_memory64(false);

    // wasmtime default: true
    c.wasm_bulk_memory(true);

    // wasmtime default: false
    c.wasm_module_linking(false);

    // wasmtime default: true
    c.wasm_multi_value(false); // ??

    // wasmtime default: depends on the arch
    // > This is true by default on x86-64, and false by default on other architectures.
    c.wasm_reference_types(false);

    // wasmtime default: false
    //
    // from wasmtime docs:
    // > When Cranelift is used as a code generation backend this will
    // > configure it to replace NaNs with a single canonical value. This
    // > is useful for users requiring entirely deterministic WebAssembly
    // > computation. This is not required by the WebAssembly spec, so it is
    // > not enabled by default.
    c.cranelift_nan_canonicalization(true);

    // c.cranelift_opt_level(Speed); ?

    c
}

impl Default for Engine {
    fn default() -> Self {
        Engine::new(&default_wasmtime_config()).unwrap()
    }
}

struct EngineInner {
    engine: wasmtime::Engine,
    module_cache: Mutex<HashMap<Cid, Module>>,
    instance_cache: Mutex<anymap::Map<dyn anymap::any::Any + Send>>,
}

impl Deref for Engine {
    type Target = wasmtime::Engine;

    fn deref(&self) -> &Self::Target {
        &self.0.engine
    }
}

impl Engine {
    /// Create a new Engine from a wasmtime config.
    pub fn new(c: &wasmtime::Config) -> anyhow::Result<Self> {
        Ok(wasmtime::Engine::new(c)?.into())
    }
}

impl From<wasmtime::Engine> for Engine {
    fn from(engine: wasmtime::Engine) -> Self {
        Engine(Arc::new(EngineInner {
            engine,
            module_cache: Default::default(),
            instance_cache: Mutex::new(anymap::Map::new()),
        }))
    }
}

struct Cache<K> {
    linker: wasmtime::Linker<InvocationData<K>>,
}

const DEFAULT_STACK_LIMIT: u32 = 100000; // todo figure out a good number

impl Engine {
    /// Instantiates and caches the Wasm modules for the bytecodes addressed by
    /// the supplied CIDs. Only uncached entries are actually fetched and
    /// instantiated. Blockstore failures and entry inexistence shortcircuit
    /// make this method return an Err immediately.
    pub fn preload<'a, BS, I>(
        &self,
        blockstore: BS,
        cids: I,
        mctx: &MachineContext,
    ) -> anyhow::Result<()>
    where
        BS: Blockstore,
        I: IntoIterator<Item = &'a Cid>,
    {
        let mut cache = self.0.module_cache.lock().expect("module_cache poisoned");
        for cid in cids {
            if cache.contains_key(cid) {
                continue;
            }
            let wasm = blockstore.get(cid)?.ok_or_else(|| {
                anyhow!(
                    "no wasm bytecode in blockstore for CID {}",
                    &cid.to_string()
                )
            })?;
            let module = self.load_raw(wasm.as_slice(), mctx)?;
            cache.insert(*cid, module);
        }
        Ok(())
    }

    /// Load some wasm code into the engine.
    pub fn load_bytecode(
        &self,
        k: &Cid,
        wasm: &[u8],
        mctx: &MachineContext,
    ) -> anyhow::Result<Module> {
        let mut cache = self.0.module_cache.lock().expect("module_cache poisoned");
        let module = match cache.get(k) {
            Some(module) => module.clone(),
            None => {
                let module = self.load_raw(wasm, mctx)?;
                cache.insert(*k, module.clone());
                module
            }
        };
        Ok(module)
    }

    fn load_raw(&self, raw_wasm: &[u8], mctx: &MachineContext) -> anyhow::Result<Module> {
        // First make sure that non-instrumented wasm is valid
        Module::validate(&self.0.engine, raw_wasm).map_err(anyhow::Error::msg)?;

        // Note: when adding debug mode support (with recorded syscall replay) don't instrument to
        // avoid breaking debug info

        use fvm_wasm_instrument::gas_metering::inject;
        use fvm_wasm_instrument::inject_stack_limiter;
        use fvm_wasm_instrument::parity_wasm::deserialize_buffer;

        let m = deserialize_buffer(raw_wasm)?;

        // stack limiter adds post/pre-ambles to call instructions; We want to do that
        // before injecting gas accounting calls to avoid this overhead in every single
        // block of code.
        let m = inject_stack_limiter(m, DEFAULT_STACK_LIMIT).map_err(anyhow::Error::msg)?;


        // inject gas metering based on a price list. This function will
        // * add a new mutable i64 global import, gas.gas_counter
        // * push a gas counter function which deduces gas from the global, and
        //   traps when gas.gas_counter is less than zero
        // * optionally push a function which wraps memory.grow instruction
        //   making it charge gas based on memory requested
        // * divide code into metered blocks, and add a call to the gas counter
        //   function before entering each metered block
        let m = inject(m, mctx.network.price_list, "gas")
            .map_err(|_| anyhow::Error::msg("injecting gas counter failed"))?;


        let wasm = m.to_bytes()?;
        let module = Module::from_binary(&self.0.engine, wasm.as_slice())?;


        Ok(module)
    }

    /// Load compiled wasm code into the engine.
    ///
    /// # Safety
    ///
    /// See [`wasmtime::Module::deserialize`] for safety information.
    pub unsafe fn load_compiled(&self, k: &Cid, compiled: &[u8]) -> anyhow::Result<Module> {
        let mut cache = self.0.module_cache.lock().expect("module_cache poisoned");
        let module = match cache.get(k) {
            Some(module) => module.clone(),
            None => {
                let module = Module::deserialize(&self.0.engine, compiled)?;
                cache.insert(*k, module.clone());
                module
            }
        };
        Ok(module)
    }

    /// Lookup a loaded wasmtime module.
    pub fn get_module(&self, k: &Cid) -> Option<Module> {
        self.0
            .module_cache
            .lock()
            .expect("module_cache poisoned")
            .get(k)
            .cloned()
    }

    /// Lookup and instantiate a loaded wasmtime module with the given store. This will cache the
    /// linker, syscalls, "pre" isntance, etc.
    pub fn get_instance<K: Kernel>(
        &self,
        store: &mut wasmtime::Store<InvocationData<K>>,
        k: &Cid,
    ) -> anyhow::Result<Option<wasmtime::Instance>> {
        let mut instance_cache = self.0.instance_cache.lock().expect("cache poisoned");

        let cache = match instance_cache.entry() {
            anymap::Entry::Occupied(e) => e.into_mut(), // todo what about gas here?
            anymap::Entry::Vacant(e) => e.insert({
                let mut linker = Linker::new(&self.0.engine);
                linker.allow_shadowing(true);

                bind_syscalls(&mut linker)?;
                Cache { linker }
            }),
        };
        cache.linker.define(
            "gas",
            GAS_COUNTER_NAME,
            store.data_mut().avail_gas_global.unwrap(),
        )?;

        let module_cache = self.0.module_cache.lock().expect("module_cache poisoned");
        let module = match module_cache.get(k) {
            Some(module) => module,
            None => return Ok(None),
        };
        let instance = cache.linker.instantiate(&mut *store, module)?;
        Ok(Some(instance))
    }

    /// Construct a new wasmtime "store" from the given kernel.
    pub fn new_store<K: Kernel>(
        &self,
        kernel: K,
        miligas: i64,
    ) -> wasmtime::Store<InvocationData<K>> {
        let mut store = wasmtime::Store::new(&self.0.engine, InvocationData::new(kernel));

        let ggtype = GlobalType::new(ValType::I64, Mutability::Var);
        let gg = Global::new(&mut store, ggtype, Val::I64(miligas))
            .expect("failed to create available_gas global");
        store.data_mut().avail_gas_global = Some(gg);

        store
    }
}
