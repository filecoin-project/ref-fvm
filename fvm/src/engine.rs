// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::any::{Any, TypeId};
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Condvar, Mutex};
use std::thread;

use anyhow::{anyhow, Context};
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_shared::error::ExitCode;
use fvm_wasm_instrument::gas_metering::GAS_COUNTER_NAME;
use num_traits::Zero;
use wasmtime::OptLevel::Speed;
use wasmtime::{
    Global, GlobalType, InstanceAllocationStrategy, Linker, Memory, MemoryType, Module, Mutability,
    Val, ValType,
};

use crate::gas::{Gas, GasTimer, WasmGasPrices};
use crate::machine::limiter::MemoryLimiter;
use crate::machine::{Machine, NetworkConfig};
use crate::syscalls::error::Abort;
use crate::syscalls::{
    bind_syscalls, charge_for_exec, charge_for_init, record_init_time, update_gas_available,
    InvocationData,
};
use crate::Kernel;

const EFFECTIVE_STACK_DEPTH: u32 = 20;

/// Container managing engines with different consensus-affecting configurations.
pub struct MultiEngine {
    engines: Mutex<HashMap<EngineConfig, EnginePool>>,
    concurrency: u32,
}

/// The proper way of getting this struct is to convert from `NetworkConfig`
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct EngineConfig {
    pub max_call_depth: u32,
    pub max_wasm_stack: u32,
    pub max_inst_memory_bytes: u64,
    pub concurrency: u32,
    pub wasm_prices: &'static WasmGasPrices,
    pub actor_redirect: Vec<(Cid, Cid)>,
}

impl From<&NetworkConfig> for EngineConfig {
    fn from(nc: &NetworkConfig) -> Self {
        EngineConfig {
            max_call_depth: nc.max_call_depth,
            max_wasm_stack: nc.max_wasm_stack,
            max_inst_memory_bytes: nc.max_inst_memory_bytes,
            wasm_prices: &nc.price_list.wasm_rules,
            actor_redirect: nc.actor_redirect.clone(),
            concurrency: 1,
        }
    }
}

impl MultiEngine {
    pub fn new(concurrency: u32) -> MultiEngine {
        if concurrency == 0 {
            panic!("concurrency must be positive");
        }
        MultiEngine {
            engines: Mutex::new(HashMap::new()),
            concurrency,
        }
    }

    pub fn get(&self, nc: &NetworkConfig) -> anyhow::Result<EnginePool> {
        let mut engines = self
            .engines
            .lock()
            .map_err(|_| anyhow::Error::msg("multiengine lock is poisoned"))?;

        let mut ec: EngineConfig = nc.into();
        ec.concurrency = self.concurrency;

        let pool = match engines.entry(ec.clone()) {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => entry.insert(EnginePool::new_default(ec)?),
        };

        Ok(pool.clone())
    }
}

impl Default for MultiEngine {
    fn default() -> Self {
        Self::new(1)
    }
}

fn wasmtime_config(ec: &EngineConfig) -> anyhow::Result<wasmtime::Config> {
    let instance_count = ec.max_call_depth + EFFECTIVE_STACK_DEPTH * ec.concurrency;
    let instance_memory_maximum_size = ec.max_inst_memory_bytes;
    if instance_memory_maximum_size % wasmtime_environ::WASM_PAGE_SIZE as u64 != 0 {
        return Err(anyhow!(
            "requested memory limit {} not a multiple of the WASM_PAGE_SIZE {}",
            instance_memory_maximum_size,
            wasmtime_environ::WASM_PAGE_SIZE
        ));
    }

    let mut c = wasmtime::Config::default();

    // wasmtime default: OnDemand
    // We want to pre-allocate all permissible memory to support the maximum allowed recursion limit.

    let mut alloc_strat_cfg = wasmtime::PoolingAllocationConfig::default();
    alloc_strat_cfg.instance_count(instance_count);

    // Adjust the maximum amount of host memory that can be committed to an instance to
    // match the static linear memory size we reserve for each slot.
    alloc_strat_cfg.instance_memory_pages(
        instance_memory_maximum_size / (wasmtime_environ::WASM_PAGE_SIZE as u64),
    );
    c.allocation_strategy(InstanceAllocationStrategy::Pooling(alloc_strat_cfg));

    // wasmtime default: true
    // We disable this as we always charge for memory regardless and `memory_init_cow` can baloon compiled wasm modules.
    c.memory_init_cow(false);

    // wasmtime default: 4GB
    c.static_memory_maximum_size(instance_memory_maximum_size);

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
    c.max_wasm_stack(4 << 20);

    // Execution cost accouting is done through wasm instrumentation,
    c.consume_fuel(false);
    c.epoch_interruption(false);

    // Disable debug-related things, wasm-instrument doesn't fix debug info
    // yet, so those aren't useful, just add overhead
    c.debug_info(false);
    c.generate_address_map(false);
    c.cranelift_debug_verifier(false);
    c.native_unwind_info(false);
    #[allow(deprecated)] // TODO https://github.com/bytecodealliance/wasmtime/issues/5037
    c.wasm_backtrace(false);
    c.wasm_reference_types(false);

    // Reiterate some defaults
    c.guard_before_linear_memory(true);
    c.parallel_compilation(true);

    #[cfg(feature = "wasmtime/async")]
    c.async_support(false);

    // Doesn't seem to have significant impact on the time it takes to load code
    // todo(M2): make sure this is guaranteed to run in linear time.
    c.cranelift_opt_level(Speed);

    Ok(c)
}

#[derive(Clone)]
struct ModuleRecord {
    module: Module,
    /// Byte size of the original Wasm.
    size: usize,
}

struct EngineInner {
    limit: Mutex<u32>,
    condv: Condvar,

    engine: wasmtime::Engine,
    pool: Arc<InstancePool>,

    /// These two fields are used used in the store constructor to avoid resolve a chicken & egg
    /// situation: We need the store before we can get the real values, but we need to create the
    /// `InvocationData` before we can make the store.
    ///
    /// Alternatively, we could use `Option`s. But then we need to unwrap everywhere.
    dummy_gas_global: Global,
    dummy_memory: Memory,

    module_cache: Mutex<HashMap<Cid, ModuleRecord>>,
    instance_cache: Mutex<HashMap<TypeId, Box<dyn Any + Send>>>,
    config: EngineConfig,

    actor_redirect: HashMap<Cid, Cid>,
}

/// EnginePool represents a limited pool of engines.
#[derive(Clone)]
pub struct EnginePool(Arc<EngineInner>);

impl EnginePool {
    /// Acquire an [`Engine`]. This method will block until an [`Engine`] is available, and will
    /// release the engine on drop.
    pub fn acquire(&self) -> Engine {
        *self
            .0
            .condv
            .wait_while(self.0.limit.lock().unwrap(), |limit| *limit == 0)
            .unwrap() -= 1;
        Engine(self.0.clone())
    }

    /// Try to acquire an [`Engine`]. Returns `None` if the call would block, or if the lock is
    /// poisoned.
    ///
    /// The [`Engine`] is released on drop.
    pub fn try_acquire(&self) -> Option<Engine> {
        self.0
            .limit
            .try_lock()
            .ok()
            .filter(|limit| **limit > 0)
            .map(|mut limit| {
                *limit -= 1;
                Engine(self.0.clone())
            })
    }

    pub fn new_default(ec: EngineConfig) -> anyhow::Result<Self> {
        EnginePool::new(&wasmtime_config(&ec)?, ec)
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

        let actor_redirect = ec.actor_redirect.iter().cloned().collect();

        Ok(EnginePool(Arc::new(EngineInner {
            limit: Mutex::new(ec.concurrency),
            condv: Condvar::new(),
            engine,
            pool: Arc::new(InstancePool::new(
                ec.max_call_depth + EFFECTIVE_STACK_DEPTH * ec.concurrency,
                ec.max_call_depth,
            )),
            dummy_memory,
            dummy_gas_global: dummy_gg,
            module_cache: Default::default(),
            instance_cache: Mutex::new(HashMap::new()),
            config: ec,
            actor_redirect,
        })))
    }
}

struct Cache<K> {
    linker: wasmtime::Linker<InvocationData<K>>,
}

/// An `Engine` represents a single, caching wasm engine. It should not be shared between concurrent
/// call stacks.
///
/// The `Engine` will be returned to the [`EnginePool`] on drop.
pub struct Engine(Arc<EngineInner>);

impl Deref for Engine {
    type Target = wasmtime::Engine;

    fn deref(&self) -> &Self::Target {
        &self.0.engine
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let mut limit = self.0.limit.lock().unwrap();
        *limit += 1;
        self.0.condv.notify_one();
    }
}

impl Engine {
    /// Loads an actor's Wasm code from the blockstore by CID, and prepares
    /// it for execution by instantiating and caching the Wasm module. This
    /// method errors if the code CID is not found in the store.
    ///
    /// Return the original byte code size.
    pub fn prepare_actor_code<BS: Blockstore>(
        &self,
        code_cid: &Cid,
        blockstore: BS,
    ) -> anyhow::Result<usize> {
        let code_cid = self.with_redirect(code_cid);
        if let Some(item) = self
            .0
            .module_cache
            .lock()
            .expect("module_cache poisoned")
            .get(code_cid)
        {
            return Ok(item.size);
        }
        let wasm = blockstore.get(code_cid)?.ok_or_else(|| {
            anyhow!(
                "no wasm bytecode in blockstore for CID {}",
                &code_cid.to_string()
            )
        })?;
        // compile and cache instantiated WASM module
        self.prepare_wasm_bytecode(code_cid, &wasm)
    }

    /// Instantiates and caches the Wasm modules for the bytecodes addressed by
    /// the supplied CIDs. Only uncached entries are actually fetched and
    /// instantiated. Blockstore failures and entry inexistence shortcircuit
    /// make this method return an Err immediately.
    ///
    /// Returns the total original byte size of the modules
    pub fn preload<'a, BS, I>(&self, blockstore: BS, cids: I) -> anyhow::Result<usize>
    where
        BS: Blockstore,
        I: IntoIterator<Item = &'a Cid>,
    {
        let mut total_size = 0usize;
        for cid in cids {
            log::trace!("preloading code CID {cid}");
            let size = self.prepare_actor_code(cid, &blockstore).with_context(|| {
                anyhow!("could not prepare actor with code CID {}", &cid.to_string())
            })?;
            total_size += size;
        }
        Ok(total_size)
    }

    fn with_redirect<'a>(&'a self, k: &'a Cid) -> &'a Cid {
        match &self.0.actor_redirect.get(k) {
            Some(cid) => cid,
            None => k,
        }
    }

    /// Loads some Wasm code into the engine and prepares it for execution.
    pub fn prepare_wasm_bytecode(&self, k: &Cid, wasm: &[u8]) -> anyhow::Result<usize> {
        let k = self.with_redirect(k);
        let mut cache = self.0.module_cache.lock().expect("module_cache poisoned");
        let size = match cache.get(k) {
            Some(item) => item.size,
            None => {
                let m = self.load_raw(wasm)?;
                let s = m.size;
                cache.insert(*k, m);
                s
            }
        };
        Ok(size)
    }

    fn load_raw(&self, raw_wasm: &[u8]) -> anyhow::Result<ModuleRecord> {
        // First make sure that non-instrumented wasm is valid
        Module::validate(&self.0.engine, raw_wasm)
            .map_err(anyhow::Error::msg)
            .with_context(|| "failed to validate actor wasm")?;

        // Note: when adding debug mode support (with recorded syscall replay) don't instrument to
        // avoid breaking debug info

        use fvm_wasm_instrument::{gas_metering, stack_limiter};

        // stack limiter adds post/pre-ambles to call instructions; We want to do that
        // before injecting gas accounting calls to avoid this overhead in every single
        // block of code.
        let raw_wasm = stack_limiter::inject(raw_wasm, self.0.config.max_wasm_stack)
            .map_err(anyhow::Error::msg)?;

        // inject gas metering based on a price list. This function will
        // * add a new mutable i64 global import, gas.gas_counter
        // * push a gas counter function which deduces gas from the global, and
        //   traps when gas.gas_counter is less than zero
        // * optionally push a function which wraps memory.grow instruction
        //   making it charge gas based on memory requested
        // * divide code into metered blocks, and add a call to the gas counter
        //   function before entering each metered block
        // * NOTE: Currently cannot instrument and charge for `table.grow` because the instruction
        //   (code `0xFC 15`) uses what parity-wasm calls the `BULK_PREFIX` but it was added later in
        //   https://github.com/WebAssembly/reference-types/issues/29 and is not recognised by the
        //   parity-wasm module parser, so the contract cannot grow the tables.
        let raw_wasm = gas_metering::inject(&raw_wasm, self.0.config.wasm_prices, "gas")
            .map_err(|_| anyhow::Error::msg("injecting gas counter failed"))?;

        let module = Module::from_binary(&self.0.engine, &raw_wasm)?;

        Ok(ModuleRecord {
            module,
            size: raw_wasm.len(),
        })
    }

    /// Load compiled wasm code into the engine.
    ///
    /// # Safety
    ///
    /// See [`wasmtime::Module::deserialize`] for safety information.
    pub unsafe fn load_compiled(&self, k: &Cid, compiled: &[u8]) -> anyhow::Result<Module> {
        let k = self.with_redirect(k);
        let mut cache = self.0.module_cache.lock().expect("module_cache poisoned");
        let module = match cache.get(k) {
            Some(m) => m.module.clone(),
            None => {
                let module = Module::deserialize(&self.0.engine, compiled)?;
                cache.insert(
                    *k,
                    ModuleRecord {
                        module: module.clone(),
                        size: compiled.len(),
                    },
                );
                module
            }
        };
        Ok(module)
    }

    /// Lookup a loaded wasmtime module.
    pub fn get_module(
        &self,
        blockstore: &impl Blockstore,
        k: &Cid,
    ) -> anyhow::Result<Option<Module>> {
        let k = self.with_redirect(k);
        match self
            .0
            .module_cache
            .lock()
            .expect("module_cache poisoned")
            .entry(*k)
        {
            Occupied(v) => Ok(Some(v.get().module.clone())),
            Vacant(v) => blockstore
                .get(k)
                .context("failed to lookup wasm module in blockstore")?
                .map(|raw_wasm| Ok(v.insert(self.load_raw(&raw_wasm)?).module.clone()))
                .transpose(),
        }
    }

    /// Lookup and instantiate a loaded wasmtime module with the given store. This will cache the
    /// linker, syscalls, etc.
    ///
    /// This returns an `Abort` as it may need to execute initialization code, charge gas, etc.
    pub fn instantiate<K: Kernel>(
        &self,
        store: &mut wasmtime::Store<InvocationData<K>>,
        k: &Cid,
    ) -> Result<Option<WasmInstance>, Abort> {
        let k = self.with_redirect(k);
        let mut instance_cache = self.0.instance_cache.lock().expect("cache poisoned");

        let type_id = TypeId::of::<K>();
        let cache: &mut Cache<K> = match instance_cache.entry(type_id) {
            Occupied(e) => &mut *e
                .into_mut()
                .downcast_mut()
                .expect("invalid instance cache entry"),
            Vacant(e) => &mut *e
                .insert({
                    let mut linker: Linker<InvocationData<K>> = Linker::new(&self.0.engine);
                    linker.allow_shadowing(true);

                    bind_syscalls(&mut linker).map_err(Abort::Fatal)?;
                    Box::new(Cache { linker })
                })
                .downcast_mut()
                .expect("invalid instance cache entry"),
        };
        let gas_global = store.data_mut().avail_gas_global;
        cache
            .linker
            .define(&store, "gas", GAS_COUNTER_NAME, gas_global)
            .context("failed to define gas counter")
            .map_err(Abort::Fatal)?;

        let mut module_cache = self.0.module_cache.lock().expect("module_cache poisoned");

        let instantiate = |store: &mut wasmtime::Store<InvocationData<K>>, module| {
            // Before we instantiate the module, we should make sure the user has sufficient gas to
            // pay for the minimum memory requirements. The module instrumentation in `inject` only
            // adds code to charge for _growing_ the memory, but not for the amount made accessible
            // initially. The limits are checked by wasmtime during instantiation, though.
            let t = charge_for_init(store, module).map_err(Abort::from_error_as_fatal)?;

            // Pre-instantiate to catch any linker errors. These are considered fatal as it means
            // the wasm module wasn't properly validated.
            let pre_instance = cache
                .linker
                .instantiate_pre(module)
                .context("failed to link actor module")?;

            // acquire resources for the instance
            self.0.pool.get()?;

            // Update the gas _just_ in case.
            update_gas_available(store)?;
            let res = pre_instance.instantiate(&mut *store);

            if let Err(e) = charge_for_exec(store) {
                self.0.pool.put();
                return Err(e);
            }

            let inst = res.map_err(|e| {
                self.0.pool.put();
                // We can't really tell what type of error happened, so we have to assume that we
                // either ran out of memory or trapped. Given that we've already type-checked the
                // module, this is the most likely case anyways. That or there'a a bug in the FVM.
                Abort::Exit(
                    ExitCode::SYS_ILLEGAL_INSTRUCTION,
                    format!("failed to instantiate module: {e}"),
                    0,
                )
            })?;

            // Record the time it took for the linker to instantiate the module.
            // This should also include everything that happens above in this method.
            // Note that this does _not_ contain the time it took the load the Wasm file,
            // which could have been cached already.
            record_init_time(store, t);

            Ok(Some(WasmInstance {
                instance: inst,
                pool: self.0.pool.clone(),
            }))
        };

        match module_cache.entry(*k) {
            Occupied(v) => instantiate(store, &v.get().module),
            Vacant(v) => match store
                .data()
                .kernel
                .machine()
                .blockstore()
                .get(k)
                .context("failed to lookup wasm module in blockstore")
                .map_err(Abort::Fatal)?
            {
                Some(raw_wasm) => instantiate(
                    store,
                    &v.insert(self.load_raw(&raw_wasm).map_err(Abort::Fatal)?)
                        .module,
                ),
                None => Ok(None),
            },
        }
    }

    /// Construct a new wasmtime "store" from the given kernel.
    pub fn new_store<K: Kernel>(&self, mut kernel: K) -> wasmtime::Store<InvocationData<K>> {
        let memory_bytes = kernel.limiter_mut().memory_used();

        let id = InvocationData {
            kernel,
            last_error: None,
            avail_gas_global: self.0.dummy_gas_global,
            last_gas_available: Gas::zero(),
            last_memory_bytes: memory_bytes,
            last_charge_time: GasTimer::start(),
            memory: self.0.dummy_memory,
        };

        let mut store = wasmtime::Store::new(&self.0.engine, id);
        let ggtype = GlobalType::new(ValType::I64, Mutability::Var);
        let gg = Global::new(&mut store, ggtype, Val::I64(0))
            .expect("failed to create available_gas global");
        store.data_mut().avail_gas_global = gg;

        fn as_wasmtime_limiter<K: Kernel>(
            data: &mut InvocationData<K>,
        ) -> &mut dyn wasmtime::ResourceLimiter {
            // SAFETY: This is safe because WasmtimeLimiter is `repr(transparent)`.
            // Unfortunately, we can't simply wrap the limiter as we need to return a reference.
            let limiter: &mut WasmtimeLimiter<K::Limiter> = unsafe {
                let limiter_ref = data.kernel.limiter_mut();
                // (debug)-assert that these types have the same layout (guaranteed by
                // `repr(transparent)`).
                debug_assert_eq!(
                    std::alloc::Layout::for_value(&*limiter_ref),
                    std::alloc::Layout::new::<WasmtimeLimiter<K::Limiter>>()
                );
                // Then cast.
                &mut *(limiter_ref as *mut K::Limiter as *mut WasmtimeLimiter<K::Limiter>)
            };
            limiter as &mut dyn wasmtime::ResourceLimiter
        }

        store.limiter(as_wasmtime_limiter);

        store
    }
}

pub struct WasmInstance {
    instance: wasmtime::Instance,
    pool: Arc<InstancePool>,
}

impl Deref for WasmInstance {
    type Target = wasmtime::Instance;

    fn deref(&self) -> &Self::Target {
        &self.instance
    }
}

impl Drop for WasmInstance {
    fn drop(&mut self) {
        self.pool.put()
    }
}

struct InstancePoolInner {
    // available resources
    avail: u32,
    // resource reservation limit; if avail < rsvp, the resource pool will boost one thread
    // to receive as many resources as it needs, while avail > 0.
    // if avail reaches 0, the resource reservation will fail.
    rsvp: u32,
    // thread id of currently boosted thread, if any
    boost: Option<thread::ThreadId>,
    // active boosts for currently boosted threads
    boosting: u32,
}

struct InstancePool {
    mx: Mutex<InstancePoolInner>,
    cv: Condvar,
}

// temporarily allow dead_code
#[allow(dead_code)]
impl InstancePool {
    fn new(avail: u32, rsvp: u32) -> InstancePool {
        InstancePool {
            mx: Mutex::new(InstancePoolInner {
                avail,
                rsvp,
                boost: None,
                boosting: 0,
            }),
            cv: Condvar::new(),
        }
    }

    fn get(&self) -> Result<(), Abort> {
        let mut guard = self.mx.lock().unwrap();

        // are we above the reserveation limit? Just acquire if that's the case.
        if guard.avail > guard.rsvp {
            if guard.boost == Some(thread::current().id()) {
                guard.boosting += 1;
            }
            guard.avail -= 1;
            return Ok(());
        }

        // is there any resource still available? boost or wait.
        if guard.avail > 0 {
            match guard.boost {
                None => {
                    guard.boost = Some(thread::current().id());
                    guard.boosting += 1;
                    guard.avail -= 1;
                    return Ok(());
                }
                Some(tid) => {
                    if tid == thread::current().id() {
                        guard.boosting += 1;
                        guard.avail -= 1;
                        return Ok(());
                    }

                    let mut reguard = self.cv.wait_while(guard, |rc| rc.avail <= rc.rsvp).unwrap();
                    reguard.avail -= 1;
                    return Ok(());
                }
            }
        }

        // we've run out of resources, bail.
        Err(Abort::Fatal(anyhow!("instance pool resources exceeded")))
    }

    fn put(&self) {
        let mut guard = self.mx.lock().unwrap();

        if guard.boost == Some(thread::current().id()) {
            guard.boosting -= 1;
            if guard.boosting == 0 {
                guard.boost = None;
            }
        }

        guard.avail += 1;
        if guard.avail > guard.rsvp {
            self.cv.notify_one();
        }
    }
}

#[repr(transparent)]
struct WasmtimeLimiter<L>(L);

impl<L: MemoryLimiter> wasmtime::ResourceLimiter for WasmtimeLimiter<L> {
    fn memory_growing(&mut self, current: usize, desired: usize, maximum: Option<usize>) -> bool {
        if maximum.map_or(false, |m| desired > m) {
            return false;
        }

        self.0.grow_instance_memory(current, desired)
    }

    fn table_growing(&mut self, current: u32, desired: u32, maximum: Option<u32>) -> bool {
        if maximum.map_or(false, |m| desired > m) {
            return false;
        }
        self.0.grow_instance_table(current, desired)
    }
}

#[cfg(test)]
mod tests {
    use wasmtime::ResourceLimiter;

    use crate::engine::WasmtimeLimiter;
    use crate::machine::limiter::MemoryLimiter;

    #[derive(Default)]
    struct Limiter {
        memory: usize,
    }
    impl MemoryLimiter for Limiter {
        fn memory_used(&self) -> usize {
            unimplemented!()
        }

        fn grow_memory(&mut self, bytes: usize) -> bool {
            self.memory += bytes;
            true
        }

        fn with_stack_frame<T, G, F, R>(_: &mut T, _: G, _: F) -> R
        where
            G: Fn(&mut T) -> &mut Self,
            F: FnOnce(&mut T) -> R,
        {
            unimplemented!()
        }
    }

    #[test]
    fn memory() {
        let mut limits = WasmtimeLimiter(Limiter::default());
        assert!(limits.memory_growing(0, 3, None));
        assert_eq!(limits.0.memory, 3);

        // The maximum in the args takes precedence.
        assert!(!limits.memory_growing(3, 4, Some(2)));
        assert_eq!(limits.0.memory, 3);

        // Increase by 2.
        assert!(limits.memory_growing(2, 4, None));
        assert_eq!(limits.0.memory, 5);
    }

    #[test]
    fn table() {
        let mut limits = WasmtimeLimiter(Limiter::default());
        assert!(limits.table_growing(0, 3, None));
        assert_eq!(limits.0.memory, 3 * 8);

        // The maximum in the args takes precedence.
        assert!(!limits.table_growing(3, 4, Some(2)));
        assert_eq!(limits.0.memory, 3 * 8);

        // Increase by 2.
        assert!(limits.table_growing(2, 4, None));
        assert_eq!(limits.0.memory, 5 * 8);
    }
}
