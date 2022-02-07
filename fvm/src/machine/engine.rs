use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use cid::Cid;
use derive_more::Deref;
use wasmtime::Module;

/// A caching wasmtime engine.
#[derive(Deref, Default, Clone)]
pub struct Engine {
    #[deref]
    engine: wasmtime::Engine,
    modules: Arc<RwLock<HashMap<Cid, Module>>>,
}

impl Engine {
    /// Create a new Engine from a wasmtime config.
    pub fn new(c: &wasmtime::Config) -> anyhow::Result<Self> {
        Ok(wasmtime::Engine::new(c)?.into())
    }
}

impl From<wasmtime::Engine> for Engine {
    fn from(engine: wasmtime::Engine) -> Self {
        Engine {
            engine,
            modules: Default::default(),
        }
    }
}

impl Engine {
    /// Lookup a loaded wasmtime module.
    pub fn get(&self, k: &Cid) -> Option<Module> {
        self.modules
            .read()
            .expect("modules poisoned")
            .get(k)
            .cloned()
    }

    /// Load some wasm code into the engine.
    pub fn load(&self, k: &Cid, wasm: &[u8]) -> anyhow::Result<Module> {
        let module = Module::from_binary(&self.engine, wasm)?;
        self.modules
            .write()
            .expect("modules poisoned")
            .insert(*k, module.clone());
        Ok(module)
    }

    /// Load compiled wasm code into the engine.
    pub unsafe fn load_compiled(&self, k: &Cid, compiled: &[u8]) -> anyhow::Result<Module> {
        let module = Module::deserialize(&self.engine, compiled)?;
        self.modules
            .write()
            .expect("modules poisoned")
            .insert(*k, module.clone());
        Ok(module)
    }
}
