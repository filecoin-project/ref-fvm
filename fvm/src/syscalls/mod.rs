// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::{anyhow, Context as _};
use num_traits::Zero;
use wasmtime::{AsContextMut, ExternType, Global, Linker, Memory, Module, Val};

use crate::call_manager::backtrace;
use crate::gas::{Gas, GasInstant, GasTimer};
use crate::kernel::ExecutionError;
use crate::machine::limiter::MemoryLimiter;
use crate::Kernel;

pub(crate) mod error;

mod actor;
mod bind;
mod context;
mod crypto;
mod debug;
mod event;
mod gas;
mod ipld;
mod network;
mod rand;
mod send;
mod sself;
mod vm;

pub(self) use context::Context;

/// Invocation data attached to a wasm "store" and available to the syscall binding.
pub struct InvocationData<K> {
    /// The kernel on which this actor is being executed.
    pub kernel: K,

    /// The last-seen syscall error. This error is considered the abort "cause" if an actor aborts
    /// after receiving this error without calling any other syscalls.
    pub last_error: Option<backtrace::Cause>,

    /// The global containing remaining available gas.
    ///
    /// The counter is injected by [fvm_wasm_instrument::gas_metering::inject] called by `Engine::load_raw`.
    pub avail_gas_global: Global,

    /// The last-set milligas limit. When `charge_for_exec` is called, we charge for the
    /// _difference_ between the current gas available (the wasm global) and the
    /// `last_milligas_available`.
    pub last_gas_available: Gas,

    /// The total size of the memory used by the execution the last time we charged gas for it.
    pub last_memory_bytes: usize,

    /// Last time we charged for gas; it can be used to correlate gas with time.
    pub last_charge_time: GasInstant,

    /// The invocation's imported "memory".
    pub memory: Memory,
}

/// Updates the global available gas in the Wasm module after a syscall, to account for any
/// gas consumption that happened on the host side.
pub fn update_gas_available(
    ctx: &mut impl AsContextMut<Data = InvocationData<impl Kernel>>,
) -> Result<(), Abort> {
    let mut ctx = ctx.as_context_mut();

    // Get current gas available.
    let avail_gas = ctx.data_mut().kernel.gas_available();
    let avail_milligas = avail_gas
        .as_milligas()
        .try_into()
        // The gas tracker guarantees that this can't be the case. If this does happen, it likely
        // means there's a serious bug (e.g., some kind of wrap-around).
        .map_err(|_| Abort::Fatal(anyhow!("available milligas exceeded i64::MAX")))?;

    // Update the wasm context to reflect this.
    let gas_global = ctx.data_mut().avail_gas_global;
    gas_global
        .set(&mut ctx, Val::I64(avail_milligas))
        .map_err(|e| Abort::Fatal(anyhow!("failed to set available gas global: {}", e)))?;

    // Finally, update the last-seen values. We'll use these values in `charge_for_exec` below.
    let data = ctx.data_mut();
    data.last_gas_available = avail_gas;
    data.last_memory_bytes = data.kernel.limiter_mut().memory_used();
    data.last_charge_time = GasTimer::start();

    Ok(())
}

/// Updates the FVM-side gas tracker with newly accrued execution gas charges.
pub fn charge_for_exec<K: Kernel>(
    ctx: &mut impl AsContextMut<Data = InvocationData<K>>,
) -> Result<(), Abort> {
    let mut ctx = ctx.as_context_mut();
    let global = ctx.data_mut().avail_gas_global;

    // Get the remaining milligas. This will go _negative_ if we run out.
    let milligas_available_wasm = global
        .get(&mut ctx)
        .i64()
        .context("failed to get wasm gas")
        .map_err(Abort::Fatal)?;

    let data = ctx.data_mut();

    // abs_diff(0) is the simplest way to get the absolute value of an i64 as a u64 without
    // overflows.
    let milligas_available_wasm_abs = milligas_available_wasm.abs_diff(0);

    // Get the exec gas to charge, taking negatives into account.
    let mut exec_gas_charge = if milligas_available_wasm < 0 {
        // If the gas remaining is negative, we charge for all remaining gas, plus `-remaining_gas`.
        // That way we actually run out.
        data.last_gas_available + Gas::from_milligas(milligas_available_wasm_abs)
    } else {
        // If it's non-negative, we charge for up-to all remaining gas. This subtraction saturates
        // at zero.
        data.last_gas_available - Gas::from_milligas(milligas_available_wasm_abs)
    };

    // Now we separate the amount of gas charged for memory; this is only makes a difference in
    // tracing. `exec_gas_charge` is the number we want to charge. If, for some reason,
    // `memory_gas_charge` exceeds `exec_gas_charge`, we just set `memory_gas_charge` to
    // `exec_gas_charge`, and set `exec_gas_charge` to zero.
    let memory_bytes = data.kernel.limiter_mut().memory_used();
    let memory_delta_bytes = memory_bytes.saturating_sub(data.last_memory_bytes);

    let mut memory_gas_charge = data.kernel.price_list().grow_memory_gas(memory_delta_bytes);
    if memory_gas_charge <= exec_gas_charge {
        exec_gas_charge -= memory_gas_charge;
    } else {
        memory_gas_charge = exec_gas_charge;
        exec_gas_charge = Gas::zero();
    }

    // Now we actually charge. If we go below 0, we run out of gas.

    let t = data
        .kernel
        .charge_gas("wasm_exec", exec_gas_charge)
        .map_err(Abort::from_error_as_fatal)?;

    // It should be okay to record time associated with Wasm execution because `charge_for_exec` is
    // called before syscalls `impl_bind_syscalls`, so the syscall timings are going to be
    // interleaved, rather than nested inside it. But we also have to make sure to reset the timer
    // after each syscall, when Wasm resumes, which happens in `update_gas_available`.
    t.stop_with(data.last_charge_time);

    if !memory_gas_charge.is_zero() {
        // Only recording time for the execution, not for the memory part, which is unknown. But we
        // could perform stomething like a multi-variate linear regression to see if the amount of
        // memory explains any of the exectuion time.
        let _ = data
            .kernel
            .charge_gas("wasm_memory_grow", memory_gas_charge)
            .map_err(Abort::from_error_as_fatal)?;
    }

    Ok(())
}

/// Charge for the initial memory and tables before a Wasm module is instantiated.
///
/// The Wasm instrumentation machinery via [fvm_wasm_instrument::gas_metering::MemoryGrowCost]
/// only charges for growing the memory _beyond_ the initial amount. It's up to us to make sure
/// the minimum memory is properly charged for.
pub fn charge_for_init<K: Kernel>(
    ctx: &mut impl AsContextMut<Data = InvocationData<K>>,
    module: &Module,
) -> crate::kernel::Result<GasTimer> {
    let min_memory_bytes = min_memory_bytes(module)?;
    let mut ctx = ctx.as_context_mut();
    let mut data = ctx.data_mut();
    let memory_gas = data.kernel.price_list().init_memory_gas(min_memory_bytes);

    // Adjust `last_memory_bytes` so that we don't charge for it again in `charge_for_exec`.
    data.last_memory_bytes += min_memory_bytes;

    if let Some(min_table_elements) = min_table_elements(module) {
        let table_gas = data.kernel.price_list().init_table_gas(min_table_elements);
        let _ = data.kernel.charge_gas("wasm_table_init", table_gas)?;
    }

    data.kernel.charge_gas("wasm_memory_init", memory_gas)
}

/// Record the time it took to initialize a module.
///
/// In practice this includes all the time elapsed since the `InvocationData` was created,
/// ie. this is the first time we'll use the `last_charge_time`.
pub fn record_init_time<K: Kernel>(
    ctx: &mut impl AsContextMut<Data = InvocationData<K>>,
    timer: GasTimer,
) {
    let mut ctx = ctx.as_context_mut();
    let data = ctx.data_mut();

    // The last charge time at this point should be when the invocation started.
    timer.stop_with(data.last_charge_time);

    // Adjust the time so the next `charge_for_exec` doesn't include what we have
    // already charged for.
    data.last_charge_time = GasTimer::start();
}

/// Get the minimum amount of memory required by a module.
fn min_memory_bytes(module: &Module) -> crate::kernel::Result<usize> {
    // NOTE: Inside wasmtime this happens slightly differently, by iterating the memory plans:
    // https://github.com/bytecodealliance/wasmtime/blob/v2.0.1/crates/runtime/src/instance/allocator/pooling.rs#L380-L403
    // However, we don't have access to that level of module runtime info, hence relying on the exported memory
    // that the `CallManager` will be looking for as well.
    if let Some(ExternType::Memory(m)) = module.get_export("memory") {
        let min_memory_bytes = m.minimum() * wasmtime_environ::WASM_PAGE_SIZE as u64;
        Ok(min_memory_bytes as usize)
    } else {
        Err(ExecutionError::Fatal(anyhow!("actor has no memory export")))
    }
}

/// Get the minimum number of table elements a module will use.
///
/// This relies on a few assumptions:
///     * That we use the default value for `InstanceLimits::tables` and only allow 1 table.
///     * That `Linker::command` will only allow them to be exported with the name "table".
fn min_table_elements(module: &Module) -> Option<u32> {
    if let Some(ExternType::Table(t)) = module.get_export("table") {
        Some(t.minimum())
    } else {
        None
    }
}

use self::bind::BindSyscall;
use self::error::Abort;

// Binds the syscall handlers so they can handle invocations
// from the actor code.
pub fn bind_syscalls(
    linker: &mut Linker<InvocationData<impl Kernel + 'static>>,
) -> anyhow::Result<()> {
    linker.bind("vm", "exit", vm::exit)?;
    linker.bind("vm", "message_context", vm::message_context)?;

    linker.bind(
        "network",
        "total_fil_circ_supply",
        network::total_fil_circ_supply,
    )?;
    linker.bind("network", "context", network::context)?;
    linker.bind("network", "tipset_cid", network::tipset_cid)?;

    linker.bind("ipld", "block_open", ipld::block_open)?;
    linker.bind("ipld", "block_create", ipld::block_create)?;
    linker.bind("ipld", "block_read", ipld::block_read)?;
    linker.bind("ipld", "block_stat", ipld::block_stat)?;
    linker.bind("ipld", "block_link", ipld::block_link)?;

    linker.bind("self", "root", sself::root)?;
    linker.bind("self", "set_root", sself::set_root)?;
    linker.bind("self", "current_balance", sself::current_balance)?;
    linker.bind("self", "self_destruct", sself::self_destruct)?;

    linker.bind("actor", "resolve_address", actor::resolve_address)?;
    linker.bind(
        "actor",
        "lookup_delegated_address",
        actor::lookup_delegated_address,
    )?;
    linker.bind("actor", "get_actor_code_cid", actor::get_actor_code_cid)?;
    linker.bind("actor", "next_actor_address", actor::next_actor_address)?;
    linker.bind("actor", "create_actor", actor::create_actor)?;
    linker.bind(
        "actor",
        "get_builtin_actor_type",
        actor::get_builtin_actor_type,
    )?;
    linker.bind(
        "actor",
        "get_code_cid_for_type",
        actor::get_code_cid_for_type,
    )?;
    linker.bind("actor", "balance_of", actor::balance_of)?;

    // Only wire this syscall when M2 native is enabled.
    #[cfg(feature = "m2-native")]
    linker.bind("actor", "install_actor", actor::install_actor)?;

    linker.bind("crypto", "verify_signature", crypto::verify_signature)?;
    linker.bind(
        "crypto",
        "recover_secp_public_key",
        crypto::recover_secp_public_key,
    )?;
    linker.bind("crypto", "hash", crypto::hash)?;
    linker.bind("crypto", "verify_seal", crypto::verify_seal)?;
    linker.bind("crypto", "verify_post", crypto::verify_post)?;
    linker.bind(
        "crypto",
        "compute_unsealed_sector_cid",
        crypto::compute_unsealed_sector_cid,
    )?;
    linker.bind(
        "crypto",
        "verify_consensus_fault",
        crypto::verify_consensus_fault,
    )?;
    linker.bind(
        "crypto",
        "verify_aggregate_seals",
        crypto::verify_aggregate_seals,
    )?;
    linker.bind(
        "crypto",
        "verify_replica_update",
        crypto::verify_replica_update,
    )?;
    linker.bind("crypto", "batch_verify_seals", crypto::batch_verify_seals)?;

    linker.bind("event", "emit_event", event::emit_event)?;

    linker.bind("rand", "get_chain_randomness", rand::get_chain_randomness)?;
    linker.bind("rand", "get_beacon_randomness", rand::get_beacon_randomness)?;

    linker.bind("gas", "charge", gas::charge_gas)?;
    linker.bind("gas", "available", gas::available)?;

    // Ok, this singled-out syscall should probably be in another category.
    linker.bind("send", "send", send::send)?;

    linker.bind("debug", "log", debug::log)?;
    linker.bind("debug", "enabled", debug::enabled)?;
    linker.bind("debug", "store_artifact", debug::store_artifact)?;

    Ok(())
}
