use std::mem;

use anyhow::{anyhow, Context as _};
use wasmtime::{AsContextMut, Global, Linker, Memory, Val};

use crate::call_manager::backtrace;
use crate::gas::Gas;
use crate::Kernel;

pub(crate) mod error;

mod actor;
mod bind;
mod context;
mod crypto;
mod debug;
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
    pub avail_gas_global: Global,

    /// The last-set milligas limit. When `charge_for_exec` is called, we charge for the
    /// _difference_ between the current gas available (the wasm global) and the
    /// `last_milligas_available`.
    pub last_milligas_available: i64,

    /// The invocation's imported "memory".
    pub memory: Memory,
}

pub fn update_gas_available(
    ctx: &mut impl AsContextMut<Data = InvocationData<impl Kernel>>,
) -> Result<(), Abort> {
    let mut ctx = ctx.as_context_mut();
    let avail_milligas = ctx.data_mut().kernel.gas_available().as_milligas();

    let gas_global = ctx.data_mut().avail_gas_global;
    gas_global
        .set(&mut ctx, Val::I64(avail_milligas))
        .map_err(|e| Abort::Fatal(anyhow!("failed to set available gas global: {}", e)))?;

    ctx.data_mut().last_milligas_available = avail_milligas;
    Ok(())
}

/// Updates the FVM-side gas tracker with newly accrued execution gas charges.
pub fn charge_for_exec(
    ctx: &mut impl AsContextMut<Data = InvocationData<impl Kernel>>,
) -> Result<(), Abort> {
    let mut ctx = ctx.as_context_mut();
    let global = ctx.data_mut().avail_gas_global;

    let milligas_available = global
        .get(&mut ctx)
        .i64()
        .context("failed to get wasm gas")
        .map_err(Abort::Fatal)?;

    // Determine milligas used, and update the gas tracker.
    let milligas_used = {
        let data = ctx.data_mut();
        let last_milligas = mem::replace(&mut data.last_milligas_available, milligas_available);
        // This should never be negative, but we might as well check.
        last_milligas.saturating_sub(milligas_available)
    };

    ctx.data_mut()
        .kernel
        .charge_gas("wasm_exec", Gas::from_milligas(milligas_used))
        .map_err(Abort::from_error_as_fatal)?;

    Ok(())
}

use self::bind::BindSyscall;
use self::error::Abort;

// Binds the syscall handlers so they can handle invocations
// from the actor code.
pub fn bind_syscalls(
    linker: &mut Linker<InvocationData<impl Kernel + 'static>>,
) -> anyhow::Result<()> {
    linker.bind("vm", "abort", vm::abort)?;
    linker.bind("vm", "context", vm::context)?;

    linker.bind("network", "base_fee", network::base_fee)?;
    linker.bind(
        "network",
        "total_fil_circ_supply",
        network::total_fil_circ_supply,
    )?;

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
    linker.bind("actor", "get_actor_code_cid", actor::get_actor_code_cid)?;
    linker.bind("actor", "new_actor_address", actor::new_actor_address)?;
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

    linker.bind("crypto", "verify_signature", crypto::verify_signature)?;
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

    linker.bind("rand", "get_chain_randomness", rand::get_chain_randomness)?;
    linker.bind("rand", "get_beacon_randomness", rand::get_beacon_randomness)?;

    linker.bind("gas", "charge", gas::charge_gas)?;

    // Ok, this singled-out syscall should probably be in another category.
    linker.bind("send", "send", send::send)?;

    linker.bind("debug", "log", debug::log)?;
    linker.bind("debug", "enabled", debug::enabled)?;
    linker.bind("debug", "store_artifact", debug::store_artifact)?;

    Ok(())
}
