use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use wasmtime::{Linker, Module};

use crate::syscalls::{bind_syscalls, InvocationData};
use crate::Kernel;

/// A caching wasmtime engine.
#[derive(Clone)]
pub struct Engine(Arc<EngineInner>);

pub fn default_wasmtime_config() -> wasmtime::Config {
    let mut c = wasmtime::Config::default();

    // c.max_wasm_stack(); https://github.com/filecoin-project/ref-fvm/issues/424

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

    c.consume_fuel(true);

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
    instances: HashMap<Cid, wasmtime::InstancePre<InvocationData<K>>>,
}

impl Engine {
    /// Instantiates and caches the Wasm modules for the bytecodes addressed by
    /// the supplied CIDs. Only uncached entries are actually fetched and
    /// instantiated. Blockstore failures and entry inexistence shortcircuit
    /// make this method return an Err immediately.
    pub fn preload<'a, BS, I>(&self, blockstore: BS, cids: I) -> anyhow::Result<()>
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
            let module = Module::from_binary(&self.0.engine, wasm.as_slice())?;
            cache.insert(*cid, module);
        }
        Ok(())
    }

    /// Load some wasm code into the engine.
    pub fn load_bytecode(&self, k: &Cid, wasm: &[u8]) -> anyhow::Result<Module> {
        let mut cache = self.0.module_cache.lock().expect("module_cache poisoned");
        let module = match cache.get(k) {
            Some(module) => module.clone(),
            None => {
                let module = Module::from_binary(&self.0.engine, wasm)?;
                cache.insert(*k, module.clone());
                module
            }
        };
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
            anymap::Entry::Occupied(e) => e.into_mut(),
            anymap::Entry::Vacant(e) => e.insert({
                let mut linker = Linker::new(&self.0.engine);
                bind_syscalls(&mut linker)?;
                Cache {
                    linker,
                    instances: HashMap::new(),
                }
            }),
        };
        let instance_pre = match cache.instances.entry(*k) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                let module_cache = self.0.module_cache.lock().expect("module_cache poisoned");
                let module = match module_cache.get(k) {
                    Some(module) => module,
                    None => return Ok(None),
                };
                // We can cache the "pre instance" because our linker only has host functions.
                let pre = cache.linker.instantiate_pre(&mut *store, module)?;
                e.insert(pre)
            }
        };
        let instance = instance_pre.instantiate(&mut *store)?;
        Ok(Some(instance))
    }

    /// Construct a new wasmtime "store" from the given kernel.
    pub fn new_store<K: Kernel>(&self, kernel: K) -> wasmtime::Store<InvocationData<K>> {
        wasmtime::Store::new(&self.0.engine, InvocationData::new(kernel))
    }
}
