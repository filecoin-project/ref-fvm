use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

use cid::Cid;
use wasmtime::{Linker, Module};

use crate::syscalls::{bind_syscalls, InvocationData};
use crate::Kernel;

/// A caching wasmtime engine.
#[derive(Clone)]
pub struct Engine(Arc<EngineInner>);

impl Default for Engine {
    fn default() -> Self {
        Engine::new(&wasmtime::Config::default()).unwrap()
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
        let engine = Engine(Arc::new(EngineInner {
            engine,
            module_cache: Default::default(),
            instance_cache: Mutex::new(anymap::Map::new()),
        }));

        #[cfg(feature = "builtin_actors")]
        engine.preload();

        engine
    }
}

struct Cache<K> {
    linker: wasmtime::Linker<InvocationData<K>>,
    instances: HashMap<Cid, wasmtime::InstancePre<InvocationData<K>>>,
}

impl Engine {
    #[cfg(feature = "builtin_actors")]
    fn preload(&self) {
        let actors = [
            (
                &*crate::builtin::SYSTEM_ACTOR_CODE_ID,
                fvm_actor_system::wasm::WASM_BINARY_BLOATY,
            ),
            (
                &*crate::builtin::INIT_ACTOR_CODE_ID,
                fvm_actor_init::wasm::WASM_BINARY_BLOATY,
            ),
            (
                &*crate::builtin::CRON_ACTOR_CODE_ID,
                fvm_actor_cron::wasm::WASM_BINARY_BLOATY,
            ),
            (
                &*crate::builtin::ACCOUNT_ACTOR_CODE_ID,
                fvm_actor_account::wasm::WASM_BINARY_BLOATY,
            ),
            (
                &*crate::builtin::POWER_ACTOR_CODE_ID,
                fvm_actor_power::wasm::WASM_BINARY_BLOATY,
            ),
            (
                &*crate::builtin::MINER_ACTOR_CODE_ID,
                fvm_actor_miner::wasm::WASM_BINARY_BLOATY,
            ),
            (
                &*crate::builtin::MARKET_ACTOR_CODE_ID,
                fvm_actor_market::wasm::WASM_BINARY_BLOATY,
            ),
            (
                &*crate::builtin::PAYCH_ACTOR_CODE_ID,
                fvm_actor_paych::wasm::WASM_BINARY_BLOATY,
            ),
            (
                &*crate::builtin::MULTISIG_ACTOR_CODE_ID,
                fvm_actor_multisig::wasm::WASM_BINARY_BLOATY,
            ),
            (
                &*crate::builtin::REWARD_ACTOR_CODE_ID,
                fvm_actor_reward::wasm::WASM_BINARY_BLOATY,
            ),
            (
                &*crate::builtin::VERIFREG_ACTOR_CODE_ID,
                fvm_actor_verifreg::wasm::WASM_BINARY_BLOATY,
            ),
        ];

        for (k, bytecode) in actors {
            self.load_bytecode(k, bytecode.expect("precompiled actor not found"))
                .expect("failed to compile built-in actor");
        }
    }

    /// Load some wasm code into the engine.
    pub fn load_bytecode(&self, k: &Cid, wasm: &[u8]) -> anyhow::Result<Module> {
        let module = Module::from_binary(&self.0.engine, wasm)?;
        self.0
            .module_cache
            .lock()
            .expect("module_cache poisoned")
            .insert(*k, module.clone());
        Ok(module)
    }

    /// Load compiled wasm code into the engine.
    pub unsafe fn load_compiled(&self, k: &Cid, compiled: &[u8]) -> anyhow::Result<Module> {
        let module = Module::deserialize(&self.0.engine, compiled)?;
        self.0
            .module_cache
            .lock()
            .expect("module_cache poisoned")
            .insert(*k, module.clone());
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
