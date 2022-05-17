use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Context};
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_wasm_instrument::gas_metering::GAS_COUNTER_NAME;
use wasmtime::OptLevel::Speed;
use wasmtime::{Global, GlobalType, Linker, Memory, MemoryType, Module, Mutability, Val, ValType};

use crate::gas::WasmGasPrices;
use crate::machine::NetworkConfig;
use crate::syscalls::{bind_syscalls, InvocationData};
use crate::Kernel;

/// A caching wasmtime engine.
#[derive(Clone)]
pub struct Engine(Arc<EngineInner>);

/// Container managing engines with different consensus-affecting configurations.
#[derive(Clone)]
pub struct MultiEngine(Arc<Mutex<HashMap<EngineConfig, Engine>>>);

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct EngineConfig {
    pub max_wasm_stack: u32,
    pub wasm_prices: &'static WasmGasPrices,
}

impl From<&NetworkConfig> for EngineConfig {
    fn from(nc: &NetworkConfig) -> Self {
        EngineConfig {
            max_wasm_stack: nc.max_wasm_stack,
            wasm_prices: &nc.price_list.wasm_rules,
        }
    }
}

impl MultiEngine {
    pub fn new() -> MultiEngine {
        MultiEngine(Arc::new(Mutex::new(HashMap::new())))
    }

    pub fn get(&self, nc: &NetworkConfig) -> anyhow::Result<Engine> {
        let mut engines = self
            .0
            .lock()
            .map_err(|_| anyhow::Error::msg("multiengine lock is poisoned"))?;

        let ec: EngineConfig = nc.into();

        let engine = match engines.entry(ec.clone()) {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => entry.insert(Engine::new_default(ec)?),
        };

        Ok(engine.clone())
    }
}

impl Default for MultiEngine {
    fn default() -> Self {
        Self::new()
    }
}

pub fn default_wasmtime_config() -> wasmtime::Config {
    let mut c = wasmtime::Config::default();

    // wasmtime default: false
    // We don't want threads, there is no way to ensure determisism
    c.wasm_threads(false);

    // wasmtime default: true
    // simd isn't supported in wasm-instrument, but if we add support there, we can probably enable this.
    // Note: stack limits may need adjusting after this is enabled
    c.wasm_simd(false);

    // wasmtime default: false
    c.wasm_multi_memory(false);

    // wasmtime default: false
    c.wasm_memory64(false);

    // wasmtime default: true
    // Note: wasm-instrument only supports this at a basic level, for M2 we will
    // need to add more advanced support
    c.wasm_bulk_memory(true);

    // wasmtime default: true
    // we should be able to enable this for M2, just need to make sure that it's
    // handled correctly in wasm-instrument
    c.wasm_multi_value(false);

    // wasmtime default: depends on the arch
    // > This is true by default on x86-64, and false by default on other architectures.
    //
    // Not supported in wasm-instrument/parity-wasm; adding support will be complicated.
    // Note: stack limits may need adjusting after this is enabled
    // NOTE: only needed when backtraces are enabled.
    #[cfg(feature = "wasmtime/wasm-backtrace")]
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

    // wasmtime default: 512KiB
    // Set to something much higher than the instrumented limiter.
    // Note: This is in bytes, while the instrumented limit is in stack elements
    c.max_wasm_stack(4 << 20).unwrap();

    // Execution cost accouting is done through wasm instrumentation,
    c.consume_fuel(false);
    c.epoch_interruption(false);

    // Disable debug-related things, wasm-instrument doesn't fix debug info
    // yet, so those aren't useful, just add overhead
    c.debug_info(false);
    c.generate_address_map(false);
    c.cranelift_debug_verifier(false);

    // Reiterate some defaults
    c.guard_before_linear_memory(true);
    c.parallel_compilation(true);

    // Doesn't seem to have significant impact on the time it takes to load code
    // todo(M2): make sure this is guaranteed to run in linear time.
    c.cranelift_opt_level(Speed);

    c
}

struct EngineInner {
    engine: wasmtime::Engine,

    /// These two fields are used used in the store constructor to avoid resolve a chicken & egg
    /// situation: We need the store before we can get the real values, but we need to create the
    /// `InvocationData` before we can make the store.
    ///
    /// Alternatively, we could use `Option`s. But then we need to unwrap everywhere.
    dummy_gas_global: Global,
    dummy_memory: Memory,

    module_cache: Mutex<HashMap<Cid, Module>>,
    instance_cache: Mutex<anymap::Map<dyn anymap::any::Any + Send>>,
    config: EngineConfig,
}

impl Deref for Engine {
    type Target = wasmtime::Engine;

    fn deref(&self) -> &Self::Target {
        &self.0.engine
    }
}

impl Engine {
    pub fn new_default(ec: EngineConfig) -> anyhow::Result<Self> {
        Engine::new(&default_wasmtime_config(), ec)
    }

    /// Create a new Engine from a wasmtime config.
    pub fn new(c: &wasmtime::Config, ec: EngineConfig) -> anyhow::Result<Self> {
        let engine = wasmtime::Engine::new(c)?;

        let mut dummy_store = wasmtime::Store::new(&engine, ());
        let gg_type = GlobalType::new(ValType::I64, Mutability::Var);
        let dummy_gg = Global::new(&mut dummy_store, gg_type, Val::I64(0))
            .expect("failed to create dummy gas global");

        let dummy_memory = Memory::new(&mut dummy_store, MemoryType::new(0, Some(0)))
            .expect("failed to create dummy memory");

        Ok(Engine(Arc::new(EngineInner {
            engine,
            dummy_memory,
            dummy_gas_global: dummy_gg,
            module_cache: Default::default(),
            instance_cache: Mutex::new(anymap::Map::new()),
            config: ec,
        })))
    }
}
struct Cache<K> {
    linker: wasmtime::Linker<InvocationData<K>>,
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
            let module = self.load_raw(wasm.as_slice())?;
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
                let module = self.load_raw(wasm)?;
                cache.insert(*k, module.clone());
                module
            }
        };
        Ok(module)
    }

    fn load_raw(&self, raw_wasm: &[u8]) -> anyhow::Result<Module> {
        // First make sure that non-instrumented wasm is valid
        Module::validate(&self.0.engine, raw_wasm)
            .map_err(anyhow::Error::msg)
            .with_context(|| "failed to validate actor wasm")?;

        // Note: when adding debug mode support (with recorded syscall replay) don't instrument to
        // avoid breaking debug info

        use fvm_wasm_instrument::gas_metering::inject;
        use fvm_wasm_instrument::inject_stack_limiter;
        use fvm_wasm_instrument::parity_wasm::deserialize_buffer;

        let m = deserialize_buffer(raw_wasm)?;

        // stack limiter adds post/pre-ambles to call instructions; We want to do that
        // before injecting gas accounting calls to avoid this overhead in every single
        // block of code.
        let m =
            inject_stack_limiter(m, self.0.config.max_wasm_stack).map_err(anyhow::Error::msg)?;

        // inject gas metering based on a price list. This function will
        // * add a new mutable i64 global import, gas.gas_counter
        // * push a gas counter function which deduces gas from the global, and
        //   traps when gas.gas_counter is less than zero
        // * optionally push a function which wraps memory.grow instruction
        //   making it charge gas based on memory requested
        // * divide code into metered blocks, and add a call to the gas counter
        //   function before entering each metered block
        let m = inject(m, self.0.config.wasm_prices, "gas")
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
            anymap::Entry::Occupied(e) => e.into_mut(),
            anymap::Entry::Vacant(e) => e.insert({
                let mut linker = Linker::new(&self.0.engine);
                linker.allow_shadowing(true);

                bind_syscalls(&mut linker)?;
                Cache { linker }
            }),
        };
        cache
            .linker
            .define("gas", GAS_COUNTER_NAME, store.data_mut().avail_gas_global)?;

        let module_cache = self.0.module_cache.lock().expect("module_cache poisoned");
        let module = match module_cache.get(k) {
            Some(module) => module,
            None => return Ok(None),
        };
        let instance = cache.linker.instantiate(&mut *store, module)?;

        Ok(Some(instance))
    }

    /// Construct a new wasmtime "store" from the given kernel.
    pub fn new_store<K: Kernel>(&self, kernel: K) -> wasmtime::Store<InvocationData<K>> {
        let id = InvocationData {
            kernel,
            last_error: None,
            avail_gas_global: self.0.dummy_gas_global,
            last_milligas_available: 0,
            memory: self.0.dummy_memory,
        };

        let mut store = wasmtime::Store::new(&self.0.engine, id);
        let ggtype = GlobalType::new(ValType::I64, Mutability::Var);
        let gg = Global::new(&mut store, ggtype, Val::I64(0))
            .expect("failed to create available_gas global");
        store.data_mut().avail_gas_global = gg;

        store
    }
}
