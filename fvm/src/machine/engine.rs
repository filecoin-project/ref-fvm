use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::collections::hash_map::Entry::{Occupied, Vacant};
use std::ops::Deref;
use std::sync::{Arc, Mutex};

use anyhow::{Context, anyhow};
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_wasm_instrument::gas_metering::GAS_COUNTER_NAME;
use fvm_wasm_instrument::parity_wasm::elements;
use wasmtime::OptLevel::Speed;
use wasmtime::{
    Global, GlobalType, Inlining, Linker, Memory, MemoryType, Module, Mutability,
    RegallocAlgorithm, Val, ValType, WasmBacktraceDetails, WasmFeatures,
};

use crate::Kernel;
use crate::gas::WasmGasPrices;
use crate::machine::NetworkConfig;
use crate::syscalls::error::Abort;
use crate::syscalls::{InvocationData, bind_syscalls};

/// A caching wasmtime engine.
#[derive(Clone)]
pub struct Engine(Arc<EngineInner>);

/// Container managing engines with different consensus-affecting configurations.
#[derive(Clone)]
pub struct MultiEngine(Arc<Mutex<HashMap<EngineConfig, Engine>>>);

/// The proper way of getting this struct is to convert from `NetworkConfig`
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct EngineConfig {
    pub max_wasm_stack: u32,
    pub wasm_prices: &'static WasmGasPrices,
    pub actor_redirect: Vec<(Cid, Cid)>,
}

impl From<&NetworkConfig> for EngineConfig {
    fn from(nc: &NetworkConfig) -> Self {
        EngineConfig {
            max_wasm_stack: nc.max_wasm_stack,
            wasm_prices: &nc.price_list.wasm_rules,
            actor_redirect: nc.actor_redirect.clone(),
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

/// Consensus-critical: the complete set of Wasm features the FVM accepts. Widening this set
/// requires a FIP and matching support in `fvm-wasm-instrument`, which meters gas and limits stack
/// depth only for the instructions it understands. The proposals we reject are listed, with
/// reasons, in `default_wasmtime_config`.
const FVM_WASM_FEATURES: WasmFeatures = WasmFeatures::empty()
    // Deterministic only with `cranelift_nan_canonicalization(true)`.
    .union(WasmFeatures::FLOATS)
    // Gas metering imports the gas counter as a mutable global.
    .union(WasmFeatures::MUTABLE_GLOBAL)
    .union(WasmFeatures::SIGN_EXTENSION)
    .union(WasmFeatures::SATURATING_FLOAT_TO_INT)
    // On by default in wasmtime and never disabled here, so it stays to keep the accepted set
    // unchanged. The actor bundles this engine runs don't use it. `fvm-wasm-instrument`'s parser
    // rejects the proposal's instructions and value types, and the padded `call_indirect` table
    // index, so its only effect on loadable code is permitting more than one table.
    .union(WasmFeatures::REFERENCE_TYPES)
    .union(WasmFeatures::BULK_MEMORY);

pub fn default_wasmtime_config() -> wasmtime::Config {
    let mut c = wasmtime::Config::default();

    // Deny everything, then allow our set, so a proposal that becomes default-on upstream stays
    // rejected instead of silently widening the actor code that loads. The explicit rejections
    // below are covered by the deny-all; they record what we turn off and why.
    c.wasm_features(WasmFeatures::all(), false);

    // wasmtime default: true
    // SIMD isn't supported in wasm-instrument. If we add support there we can probably enable this,
    // but stack limits may need adjusting.
    c.wasm_simd(false);
    c.wasm_relaxed_simd(false);

    // wasmtime default: false
    // Supports wide instructions (https://github.com/WebAssembly/wide-arithmetic). We'd like this,
    // but we'll need to (a) update our gas instrumentation logic to support it and (b) make sure our
    // build pipeline can take advantage of it.
    c.wasm_wide_arithmetic(false);

    // wasmtime default: true
    // We don't support the return_call_* functions.
    c.wasm_tail_call(false);

    // wasmtime default: true
    // We support exactly one 32-bit linear memory with 64KiB pages.
    c.wasm_multi_memory(false);
    c.wasm_memory64(false);
    c.wasm_custom_page_sizes(false);

    // wasmtime default: true
    // We should be able to enable this, but need to make sure it's handled correctly in
    // wasm-instrument.
    c.wasm_multi_value(false);

    // wasmtime default: true
    // We'll probably enable this in the future, but that requires a FIP.
    c.wasm_extended_const(false);

    // wasmtime default: true (function references, and GC; exceptions with the "gc" feature)
    // New types, control flow and traps (e.g. `NullReference`) that wasm-instrument and our
    // trap-to-exit-code mapping don't model. The setters for function references and exceptions
    // exist only with the "gc" feature, which we don't build.
    c.wasm_gc(false);
    c.wasm_features(WasmFeatures::FUNCTION_REFERENCES, false);
    c.wasm_features(WasmFeatures::EXCEPTIONS, false);

    // wasmtime default: false
    // Typed continuations: control flow wasm-instrument doesn't model.
    c.wasm_stack_switching(false);

    // wasmtime default: true for threads ("threads" feature only), false for the rest
    // Shared memory and atomics are nondeterministic across threads. The setter exists only with
    // the "threads" feature, which we don't build.
    c.wasm_features(WasmFeatures::THREADS, false);
    c.wasm_shared_everything_threads(false);
    c.shared_memory(false);

    c.wasm_features(FVM_WASM_FEATURES, true);

    // wasmtime default: false
    // Branch hints are a codegen input carried in a custom section; ignore them.
    c.wasm_branch_hinting(false);

    // A codegen tunable, not a feature, so it cannot live in the mask: relaxed SIMD permits
    // implementation-defined results, and this forces deterministic lowering if it is ever enabled.
    c.relaxed_simd_deterministic(true);

    // wasmtime default: true
    // We disable this as we always charge for memory regardless and `memory_init_cow` can balloon
    // compiled wasm modules.
    c.memory_init_cow(false);

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

    // We don't use async, but wasmtime unconditionally rejects a `max_wasm_stack` larger than the
    // `async_stack_size` (default 2MiB), so raise the latter to keep the stack limit above.
    c.async_stack_size(8 << 20);

    // Execution cost accouting is done through wasm instrumentation,
    c.consume_fuel(false);
    c.epoch_interruption(false);

    // Disable debug-related things, wasm-instrument doesn't fix debug info
    // yet, so those aren't useful, just add overhead
    c.debug_info(false);
    c.generate_address_map(false);
    c.cranelift_debug_verifier(false);
    c.native_unwind_info(false);
    c.wasm_backtrace_max_frames(None);
    c.wasm_backtrace_details(WasmBacktraceDetails::Disable);

    // Reiterate some defaults
    c.guard_before_linear_memory(true);
    c.parallel_compilation(true);

    // wasmtime defaults on 64-bit hosts. A 4GiB reservation plus guard region lets compiled code
    // elide bounds checks and rely on signal-based traps for out-of-bounds access. Several
    // wasmtime advisories (e.g. GHSA-6wgr-89rj-399p) only apply with a zero guard or a smaller
    // reservation.
    c.memory_reservation(1 << 32);
    c.memory_guard_size(32 << 20);
    c.signals_based_traps(true);

    // wasmtime defaults. Inlining and the register allocator shape native stack frames, and the
    // native stack limit (`max_wasm_stack`) must never be reached before the instrumented one.
    c.compiler_inlining(Inlining::No);
    c.cranelift_regalloc_algorithm(RegallocAlgorithm::Backtracking);

    // Note: Caching is disabled by default.
    // If we add the "wasmtime/cache" feature in the future,
    // we would explicitly set c.disable_cache() here.

    // Doesn't seem to have significant impact on the time it takes to load code
    // todo(M2): make sure this is guaranteed to run in linear time.
    c.cranelift_opt_level(Speed);

    // Use traditional unix signal handlers instead of Mach ports on MacOS. The Mach ports signal
    // handlers don't appear to be capturing all SIGILL signals (when run under lotus) and we're not
    // entirely sure why. Upstream documentation indicates that this could be due to the use of
    // `fork`, but we're only using threads, not subprocesses.
    //
    // The downside to using traditional signal handlers is that this may interfere with some
    // debugging tools. But we'll just have to live with that.
    c.macos_use_mach_ports(false);

    // wasmtime default: true
    // TODO: Consider disabling this to make performance more deterministic. But we benchmarked with
    // it on, so we leave it on for now.
    // https://github.com/filecoin-project/ref-fvm/issues/2129
    // c.table_lazy_init(false);

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
    instance_cache: Mutex<HashMap<TypeId, Box<dyn Any + Send>>>,
    config: EngineConfig,

    actor_redirect: HashMap<Cid, Cid>,
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

        let actor_redirect = ec.actor_redirect.iter().cloned().collect();

        Ok(Engine(Arc::new(EngineInner {
            engine,
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
            let cid = self.with_redirect(cid);
            if cache.contains_key(cid) {
                continue;
            }
            let wasm = blockstore
                .get(cid)?
                .ok_or_else(|| anyhow!("no wasm bytecode in blockstore for CID {}", cid))?;
            let module = self.load_raw(wasm.as_slice())?;
            cache.insert(*cid, module);
        }
        Ok(())
    }

    fn with_redirect<'a>(&'a self, k: &'a Cid) -> &'a Cid {
        match &self.0.actor_redirect.get(k) {
            Some(cid) => cid,
            None => k,
        }
    }

    /// Load some wasm code into the engine.
    pub fn load_bytecode(&self, k: &Cid, wasm: &[u8]) -> anyhow::Result<Module> {
        let k = self.with_redirect(k);
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
        let mut m = inject(m, self.0.config.wasm_prices, "gas")
            .map_err(|_| anyhow::Error::msg("injecting gas counter failed"))?;

        // Work around #602. Remove this once paritytech/parity-wasm#331 is merged and bubbled.
        fix_wasm_sections(&mut m);

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
        let k = self.with_redirect(k);
        let mut cache = self.0.module_cache.lock().expect("module_cache poisoned");
        let module = match cache.get(k) {
            Some(module) => module.clone(),
            None => {
                let module = unsafe { Module::deserialize(&self.0.engine, compiled)? };
                cache.insert(*k, module.clone());
                module
            }
        };
        Ok(module)
    }

    /// Lookup a loaded wasmtime module.
    pub fn get_module(&self, k: &Cid) -> Option<Module> {
        let k = self.with_redirect(k);
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

                    bind_syscalls(&mut linker)?;
                    Box::new(Cache { linker })
                })
                .downcast_mut()
                .expect("invalid instance cache entry"),
        };
        let gas_global = store.data_mut().avail_gas_global;
        cache
            .linker
            .define(&store, "gas", GAS_COUNTER_NAME, gas_global)
            .map_err(|e| Abort::Fatal(e.context("failed to define gas counter").into()))?;

        let module_cache = self.0.module_cache.lock().expect("module_cache poisoned");
        let module = match module_cache.get(k) {
            Some(module) => module,
            None => return Ok(None),
        };

        let pre_instance = cache
            .linker
            .instantiate_pre(module)
            .map_err(|e| e.context("failed to link actor module"))?;
        let instance = pre_instance.instantiate(&mut *store)?;

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

// Workaround for https://github.com/filecoin-project/ref-fvm/issues/602
//
// This removes the out-of-order data count section, if it exists, and re-inserts it (with the
// correct data-count).
fn fix_wasm_sections(module: &mut elements::Module) {
    module
        .sections_mut()
        .retain(|sec| !matches!(sec, elements::Section::DataCount(_)));
    if let Some(data_count) = module.data_section().map(|data| data.entries().len()) {
        module
            .insert_section(elements::Section::DataCount(data_count as u32))
            .expect("section wasn't deleted");
    }
}

#[cfg(test)]
mod tests {
    use fvm_shared::error::ExitCode;
    use fvm_shared::version::NetworkVersion;

    use super::{Engine, EngineConfig, FVM_WASM_FEATURES, default_wasmtime_config};
    use crate::machine::NetworkConfig;
    use crate::syscalls::error::Abort;

    fn validates(engine: &wasmtime::Engine, wasm: &[u8]) -> bool {
        wasmtime::Module::validate(engine, wasm).is_ok()
    }

    // The accepted Wasm feature set is consensus-critical and must not drift across wasmtime
    // upgrades. This pins both sides: everything we accept and every proposal we reject.
    #[test]
    fn accepted_wasm_features_are_fixed() {
        let engine = wasmtime::Engine::new(&default_wasmtime_config()).unwrap();
        let validates =
            |engine: &wasmtime::Engine, wat: &str| validates(engine, &wat::parse_str(wat).unwrap());

        const CALL_REF: &str =
            r#"(module (type $t (func)) (func (param $p (ref $t)) (call_ref $t (local.get $p))))"#;

        // Accepted: MVP plus funcref reference-types, bulk-memory, sign-extension and non-trapping
        // float-to-int.
        for m in [
            r#"(module (func (export "f")))"#,
            r#"(module (type $t (func)) (table 1 funcref) (func (export "f") (call_indirect (type $t) (i32.const 0))))"#,
            r#"(module (func (result funcref) (ref.null func)))"#,
            r#"(module (table 1 funcref) (table 1 funcref))"#,
            r#"(module (memory 1) (func (memory.fill (i32.const 0) (i32.const 0) (i32.const 0))))"#,
            r#"(module (func (param i32) (result i32) (i32.extend8_s (local.get 0))))"#,
            r#"(module (func (param f32) (result i32) (i32.trunc_sat_f32_s (local.get 0))))"#,
        ] {
            assert!(validates(&engine, m), "should be accepted: {m}");
        }

        // Rejected: one module per disabled proposal. `global.get` of a local global in a constant
        // expression is gated by wasmtime under GC (the spec files it under extended-const); both
        // are off. `externref` needs the GC runtime we build without.
        for m in [
            CALL_REF,
            r#"(module (type $s (struct (field i32))))"#,
            r#"(module (tag $e))"#,
            r#"(module (func try catch_all end))"#,
            r#"(module (global i32 (i32.const 1)) (global i32 (global.get 0)))"#,
            r#"(module (global externref (ref.null extern)))"#,
            r#"(module (func (result v128) (v128.const i32x4 0 0 0 0)))"#,
            r#"(module (func $f) (func (return_call $f)))"#,
            r#"(module (func (result i32 i32) (i32.const 0) (i32.const 1)))"#,
            r#"(module (memory 1) (memory 1))"#,
            r#"(module (memory i64 1))"#,
            r#"(module (memory 1 1 (pagesize 1)))"#,
            r#"(module (global i32 (i32.add (i32.const 1) (i32.const 2))))"#,
            r#"(module (func (param i64 i64 i64 i64) (result i64 i64) (i64.add128 (local.get 0) (local.get 1) (local.get 2) (local.get 3))))"#,
            r#"(module (memory 1 1 shared))"#,
        ] {
            assert!(!validates(&engine, m), "should be rejected: {m}");
        }

        // Control: the same `call_ref` module validates once function references are re-enabled,
        // so the rejection above is caused by our configuration.
        let mut permissive = default_wasmtime_config();
        permissive.wasm_features(wasmtime::WasmFeatures::FUNCTION_REFERENCES, true);
        let permissive = wasmtime::Engine::new(&permissive).unwrap();
        assert!(validates(&permissive, CALL_REF));
    }

    // Some toolchains encode `call_indirect`'s table index (always 0) as a 5-byte padded LEB128
    // rather than the single `0x00` byte the MVP requires. Only the reference-types proposal
    // permits the padded form, so wasmtime validates it. The actors this engine runs use the
    // minimal form, and `fvm-wasm-instrument`'s parser rejects the padded one, so such a module
    // fails to load. The text format can't express the padding, so this assembles the module by
    // hand.
    #[test]
    fn padded_call_indirect_table_index_is_rejected_on_load() {
        // Equivalent to:
        //   (module
        //     (type (func))
        //     (table 1 funcref)
        //     (func (call_indirect (type 0) (i32.const 0))))
        // with the bytes of `call_indirect`'s table index supplied by the caller.
        fn call_indirect_module(table_index: &[u8]) -> Vec<u8> {
            fn section(id: u8, content: &[u8]) -> Vec<u8> {
                // A single-byte LEB128 length is enough for these sections.
                assert!(content.len() < 0x80);
                [&[id, content.len() as u8], content].concat()
            }
            let body = [
                &[0x00][..],   // no locals
                &[0x41, 0x00], // i32.const 0
                &[0x11, 0x00], // call_indirect, type index 0
                table_index,   // call_indirect, table index
                &[0x0b],       // end
            ]
            .concat();
            assert!(body.len() < 0x80);
            [
                &b"\0asm"[..],
                &[0x01, 0x00, 0x00, 0x00],                 // version 1
                &section(0x01, &[0x01, 0x60, 0x00, 0x00]), // types: 1, func [] -> []
                &section(0x03, &[0x01, 0x00]),             // functions: 1, type 0
                &section(0x04, &[0x01, 0x70, 0x00, 0x01]), // tables: 1, funcref, min 1
                &section(0x0a, &[&[0x01, body.len() as u8][..], &body].concat()), // code: 1 body
            ]
            .concat()
        }

        let minimal = call_indirect_module(&[0x00]);
        let padded = call_indirect_module(&[0x80, 0x80, 0x80, 0x80, 0x00]);

        let ec = EngineConfig::from(&NetworkConfig::new(NetworkVersion::V17));
        let engine = Engine::new_default(ec.clone()).unwrap();
        assert!(validates(&engine, &minimal));
        assert!(validates(&engine, &padded));
        assert!(engine.load_raw(&minimal).is_ok());
        assert!(engine.load_raw(&padded).is_err());

        // Control: without reference types the padded form is rejected and the minimal one isn't.
        let mut mvp = default_wasmtime_config();
        mvp.wasm_features(FVM_WASM_FEATURES, false).wasm_features(
            FVM_WASM_FEATURES - wasmtime::WasmFeatures::REFERENCE_TYPES,
            true,
        );
        let mvp = wasmtime::Engine::new(&mvp).unwrap();
        assert!(validates(&mvp, &minimal));
        assert!(!validates(&mvp, &padded));
    }

    // Every engine setting that can affect execution semantics, trap behaviour or native stack
    // use, read back from the engine wasmtime actually built. Settings we leave at their wasmtime
    // default are asserted too, so a default that moves upstream fails here rather than shipping.
    #[test]
    fn effective_engine_config_is_fixed() {
        use wasmtime::{Inlining, OptLevel, RegallocAlgorithm};

        let e = wasmtime::Engine::new(&default_wasmtime_config()).unwrap();

        assert_eq!(e.get_wasm_features(), FVM_WASM_FEATURES);
        assert!(!e.get_wasm_branch_hinting());
        assert!(!e.get_shared_memory());

        assert_eq!(e.get_cranelift_nan_canonicalization(), Some(true));
        assert!(e.get_relaxed_simd_deterministic());
        assert_eq!(e.get_cranelift_opt_level(), Some(OptLevel::Speed));
        assert_eq!(
            e.get_cranelift_regalloc_algorithm(),
            Some(RegallocAlgorithm::Backtracking)
        );
        assert_eq!(e.get_compiler_inlining(), Inlining::No);
        assert_eq!(e.get_cranelift_debug_verifier(), Some(false));

        assert!(e.get_signals_based_traps());
        assert_eq!(e.get_memory_reservation(), 1 << 32);
        assert_eq!(e.get_memory_guard_size(), 32 << 20);
        assert!(e.get_guard_before_linear_memory());
        // Left at the wasmtime default. Moving is only needed when growth exceeds the reservation,
        // which a 32-bit memory never does with a 4GiB reservation.
        assert!(e.get_memory_may_move());
        assert!(!e.get_memory_init_cow());
        assert!(e.get_table_lazy_init());
        assert_eq!(e.get_max_wasm_stack(), 4 << 20);

        assert!(!e.get_consume_fuel());
        assert!(!e.get_epoch_interruption());
        assert!(!e.get_debug_info());
        assert!(!e.get_guest_debug());
        assert!(!e.get_generate_address_map());
        assert_eq!(e.get_native_unwind_info(), Some(false));
        assert_eq!(e.get_wasm_backtrace_max_frames(), 0);

        // Instances are allocated on demand, with no per-module memory or table limits beyond
        // those the feature set imposes.
        assert!(e.get_pooling_config().is_none());
    }

    // Each wasm trap the FVM maps to an actor exit, rather than a fatal error, run through the
    // same instrumentation and error conversion as an actor invocation. Bulk memory and table
    // bounds checks are generated code, so these also pin the trap each one raises.
    #[test]
    fn wasm_traps() {
        use fvm_wasm_instrument::gas_metering::GAS_COUNTER_NAME;
        use wasmtime::{Global, GlobalType, Mutability, Val, ValType};

        let ec = EngineConfig::from(&NetworkConfig::new(NetworkVersion::V17));
        let engine = Engine::new_default(ec).unwrap();

        let invoke = |body: &str| -> Result<u32, Abort> {
            let wat = format!(
                r#"(module
                     (type $t (func (result i32)))
                     (memory (export "memory") 1)
                     (table 3 funcref)
                     (elem (i32.const 2) $other)
                     (func $other (param i32))
                     (func (export "invoke") (param $x i32) (result i32)
                       {body}))"#
            );
            let module = engine.load_raw(&wat::parse_str(wat).unwrap()).unwrap();
            let mut store = wasmtime::Store::new(&engine, ());
            let gas = Global::new(
                &mut store,
                GlobalType::new(ValType::I64, Mutability::Var),
                Val::I64(i64::MAX),
            )
            .unwrap();
            let mut linker = wasmtime::Linker::new(&engine);
            linker.define(&store, "gas", GAS_COUNTER_NAME, gas).unwrap();
            let instance = linker.instantiate(&mut store, &module).unwrap();
            let invoke = instance
                .get_typed_func::<(u32,), u32>(&mut store, "invoke")
                .unwrap();
            Ok(invoke.call(&mut store, (0,))?)
        };

        // Control: the module itself loads and runs.
        assert_eq!(invoke("(i32.const 0)").unwrap(), 0);

        for body in [
            // MemoryOutOfBounds
            "(i32.load (i32.const 65536))",
            "(memory.fill (i32.const 65535) (i32.const 0) (i32.const 2)) (i32.const 0)",
            "(memory.copy (i32.const 0) (i32.const 65535) (i32.const 2)) (i32.const 0)",
            // TableOutOfBounds
            "(call_indirect (type $t) (i32.const 3))",
            // IndirectCallToNull
            "(call_indirect (type $t) (i32.const 0))",
            // BadSignature
            "(call_indirect (type $t) (i32.const 2))",
            // BadConversionToInteger
            "(i32.trunc_f32_s (f32.const nan))",
            // IntegerDivisionByZero
            "(i32.div_s (i32.const 1) (i32.const 0))",
            // IntegerOverflow
            "(i32.div_s (i32.const 0x80000000) (i32.const -1))",
            // UnreachableCodeReached
            "(unreachable)",
        ] {
            match invoke(body) {
                Err(Abort::Exit(code, _)) => {
                    assert_eq!(code, ExitCode::SYS_ILLEGAL_INSTRUCTION, "{body}")
                }
                other => panic!("{body}: expected an illegal instruction exit, got {other:?}"),
            }
        }
    }
}
