// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::{anyhow, Context as _};
use num_traits::Zero;
use wasmtime::{AsContext, AsContextMut, ExternType, Global, Module, Val};

use crate::call_manager::backtrace;
use crate::gas::{Gas, GasInstant, GasTimer, WasmGasPrices};
use crate::kernel::filecoin::{DefaultFilecoinKernel, FilecoinKernel};
use crate::kernel::{
    ActorOps, CryptoOps, DebugOps, EventOps, ExecutionError, IpldBlockOps, MessageOps, NetworkOps,
    RandomnessOps, SelfOps, SendOps, SyscallHandler, UpgradeOps,
};

use crate::machine::limiter::MemoryLimiter;
use crate::{DefaultKernel, Kernel};

pub(crate) mod error;

mod actor;
mod context;
mod crypto;
mod debug;
mod event;
mod filecoin;
mod gas;
mod ipld;
mod linker;
mod network;
mod rand;
mod send;
mod sself;
mod vm;

pub use context::{Context, Memory};
pub use error::Abort;
pub use linker::{ControlFlow, Linker};

pub use linker::{IntoControlFlow, Syscall};

/// Invocation data attached to a wasm "store" and available to the syscall binding.
pub(crate) struct InvocationData<K> {
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
    pub memory: wasmtime::Memory,

    pub wasm_prices: &'static WasmGasPrices,
}

/// Updates the global available gas in the Wasm module after a syscall, to account for any
/// gas consumption that happened on the host side.
pub(crate) fn update_gas_available(
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
pub(crate) fn charge_for_exec<K: Kernel>(
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

    let mut memory_gas_charge = data.wasm_prices.grow_memory_gas(memory_delta_bytes);
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
    // called before syscalls `impl_link_syscalls`, so the syscall timings are going to be
    // interleaved, rather than nested inside it. But we also have to make sure to reset the timer
    // after each syscall, when Wasm resumes, which happens in `update_gas_available`.
    t.stop_with(data.last_charge_time);

    if !memory_gas_charge.is_zero() {
        // Only recording time for the execution, not for the memory part, which is unknown. But we
        // could perform stomething like a multi-variate linear regression to see if the amount of
        // memory explains any of the exectuion time.
        data.kernel
            .charge_gas("wasm_memory_grow", memory_gas_charge)
            .map_err(Abort::from_error_as_fatal)?;
    }

    Ok(())
}

pub(crate) fn charge_syscall_gas(
    ctx: &mut impl AsContext<Data = InvocationData<impl Kernel>>,
) -> Result<(), Abort> {
    let data = ctx.as_context().data();
    data.kernel
        .charge_gas("OnSyscall", data.wasm_prices.host_call_cost)
        .map_err(Abort::from_error_as_fatal)?;
    Ok(())
}

/// Charge for the initial memory and tables before a Wasm module is instantiated.
///
/// The Wasm instrumentation machinery via [fvm_wasm_instrument::gas_metering::MemoryGrowCost]
/// only charges for growing the memory _beyond_ the initial amount. It's up to us to make sure
/// the minimum memory is properly charged for.
pub(crate) fn charge_for_init<K: Kernel>(
    ctx: &mut impl AsContextMut<Data = InvocationData<K>>,
    module: &Module,
) -> crate::kernel::Result<GasTimer> {
    let min_memory_bytes = min_memory_bytes(module)?;
    let mut ctx = ctx.as_context_mut();
    let data = ctx.data_mut();
    let memory_gas = data.wasm_prices.init_memory_gas(min_memory_bytes);

    // Adjust `last_memory_bytes` so that we don't charge for it again in `charge_for_exec`.
    data.last_memory_bytes += min_memory_bytes;

    if let Some(min_table_elements) = min_table_elements(module) {
        let table_gas = data.wasm_prices.init_table_gas(min_table_elements);
        data.kernel.charge_gas("wasm_table_init", table_gas)?;
    }

    data.kernel.charge_gas("wasm_memory_init", memory_gas)
}

/// Record the time it took to initialize a module.
///
/// In practice this includes all the time elapsed since the `InvocationData` was created,
/// ie. this is the first time we'll use the `last_charge_time`.
pub(crate) fn record_init_time<K: Kernel>(
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
        let min_memory_bytes = m.minimum() * wasmtime_environ::Memory::DEFAULT_PAGE_SIZE as u64;
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

impl<K> SyscallHandler<K> for DefaultKernel<K::CallManager>
where
    K: Kernel
        + ActorOps
        + IpldBlockOps
        + SendOps
        + UpgradeOps
        + CryptoOps
        + DebugOps
        + EventOps
        + MessageOps
        + NetworkOps
        + RandomnessOps
        + SelfOps,
{
    fn link_syscalls(linker: &mut Linker<K>) -> anyhow::Result<()> {
        linker.link_syscall("vm", "exit", vm::exit)?;
        linker.link_syscall("vm", "message_context", vm::message_context)?;

        linker.link_syscall("network", "context", network::context)?;
        linker.link_syscall("network", "tipset_cid", network::tipset_cid)?;

        linker.link_syscall("ipld", "block_open", ipld::block_open)?;
        linker.link_syscall("ipld", "block_create", ipld::block_create)?;
        linker.link_syscall("ipld", "block_read", ipld::block_read)?;
        linker.link_syscall("ipld", "block_stat", ipld::block_stat)?;
        linker.link_syscall("ipld", "block_link", ipld::block_link)?;

        linker.link_syscall("self", "root", sself::root)?;
        linker.link_syscall("self", "set_root", sself::set_root)?;
        linker.link_syscall("self", "current_balance", sself::current_balance)?;
        linker.link_syscall("self", "self_destruct", sself::self_destruct)?;

        linker.link_syscall("actor", "resolve_address", actor::resolve_address)?;
        linker.link_syscall(
            "actor",
            "lookup_delegated_address",
            actor::lookup_delegated_address,
        )?;
        linker.link_syscall("actor", "get_actor_code_cid", actor::get_actor_code_cid)?;
        linker.link_syscall("actor", "next_actor_address", actor::next_actor_address)?;
        linker.link_syscall("actor", "create_actor", actor::create_actor)?;
        if cfg!(feature = "upgrade-actor") {
            // We disable/enable with the feature, but we always compile this code to ensure we don't
            // accidentally break it.
            linker.link_syscall("actor", "upgrade_actor", actor::upgrade_actor)?;
        }
        linker.link_syscall(
            "actor",
            "get_builtin_actor_type",
            actor::get_builtin_actor_type,
        )?;
        linker.link_syscall(
            "actor",
            "get_code_cid_for_type",
            actor::get_code_cid_for_type,
        )?;
        linker.link_syscall("actor", "balance_of", actor::balance_of)?;

        // Only wire this syscall when M2 native is enabled.
        if cfg!(feature = "m2-native") {
            linker.link_syscall("actor", "install_actor", actor::install_actor)?;
        }
        #[cfg(feature = "verify-signature")]
        linker.link_syscall("crypto", "verify_signature", crypto::verify_signature)?;
        linker.link_syscall(
            "crypto",
            "verify_bls_aggregate",
            crypto::verify_bls_aggregate,
        )?;
        linker.link_syscall(
            "crypto",
            "recover_secp_public_key",
            crypto::recover_secp_public_key,
        )?;
        linker.link_syscall("crypto", "hash", crypto::hash)?;

        linker.link_syscall("event", "emit_event", event::emit_event)?;

        linker.link_syscall("rand", "get_chain_randomness", rand::get_chain_randomness)?;
        linker.link_syscall("rand", "get_beacon_randomness", rand::get_beacon_randomness)?;

        linker.link_syscall("gas", "charge", gas::charge_gas)?;
        linker.link_syscall("gas", "available", gas::available)?;

        // Ok, this singled-out syscall should probably be in another category.
        linker.link_syscall("send", "send", send::send)?;

        linker.link_syscall("debug", "log", debug::log)?;
        linker.link_syscall("debug", "enabled", debug::enabled)?;
        linker.link_syscall("debug", "store_artifact", debug::store_artifact)?;

        Ok(())
    }
}

impl<K> SyscallHandler<K> for DefaultFilecoinKernel<K::CallManager>
where
    K: FilecoinKernel
        + ActorOps
        + SendOps
        + UpgradeOps
        + IpldBlockOps
        + CryptoOps
        + DebugOps
        + EventOps
        + MessageOps
        + NetworkOps
        + RandomnessOps
        + SelfOps,
{
    fn link_syscalls(linker: &mut Linker<K>) -> anyhow::Result<()> {
        DefaultKernel::<K::CallManager>::link_syscalls(linker)?;

        // Bind the circulating supply call.
        linker.link_syscall(
            "network",
            "total_fil_circ_supply",
            filecoin::total_fil_circ_supply,
        )?;

        // Now bind the crypto syscalls.
        linker.link_syscall(
            "crypto",
            "compute_unsealed_sector_cid",
            filecoin::compute_unsealed_sector_cid,
        )?;
        linker.link_syscall("crypto", "verify_post", filecoin::verify_post)?;
        linker.link_syscall(
            "crypto",
            "verify_consensus_fault",
            filecoin::verify_consensus_fault,
        )?;
        linker.link_syscall(
            "crypto",
            "verify_aggregate_seals",
            filecoin::verify_aggregate_seals,
        )?;
        linker.link_syscall(
            "crypto",
            "verify_replica_update",
            filecoin::verify_replica_update,
        )?;
        linker.link_syscall("crypto", "batch_verify_seals", filecoin::batch_verify_seals)?;

        Ok(())
    }
}
