use wasmtime::Linker;

use crate::{kernel::Result, Kernel};

mod actor;
mod context;
mod crypto;
mod gas;
mod ipld;
mod message;
mod network;
mod rand;
mod sself;
mod validation;

pub(self) use context::Context;

/// The maximum supported CID size. (SPEC_AUDIT)
pub const MAX_CID_LEN: usize = 100;

// Binds the syscall handlers so they can handle invocations
// from the actor code.
//
// TODO try to fix the static lifetime here. I want to tell the compiler that
//  the Kernel will live as long as the Machine and the Linker.
pub fn bind_syscalls<K: Kernel + 'static>(linker: &mut Linker<K>) -> Result<()> {
    linker.func_wrap("ipld", "get_root", ipld::get_root)?;
    linker.func_wrap("ipld", "set_root", ipld::set_root)?;
    linker.func_wrap("ipld", "open", ipld::open)?;
    linker.func_wrap("ipld", "create", ipld::create)?;
    linker.func_wrap("ipld", "read", ipld::read)?;
    linker.func_wrap("ipld", "stat", ipld::stat)?;
    linker.func_wrap("ipld", "cid", ipld::cid)?;

    linker.func_wrap(
        "validation",
        "accept_any",
        validation::validate_immediate_caller_accept_any,
    )?;
    linker.func_wrap(
        "validation",
        "accept_addrs",
        validation::validate_immediate_caller_addr_one_of,
    )?;
    linker.func_wrap(
        "validation",
        "accept_types",
        validation::validate_immediate_caller_type_one_of,
    )?;

    linker.func_wrap("self", "root", sself::root)?;
    linker.func_wrap("self", "set_root", sself::set_root)?;
    linker.func_wrap("self", "current_balance", sself::current_balance)?;
    linker.func_wrap("self", "self_destruct", sself::self_destruct)?;

    linker.func_wrap("message", "caller", message::caller)?;
    linker.func_wrap("message", "receiver", message::receiver)?;
    linker.func_wrap("message", "method_number", message::method_number)?;
    linker.func_wrap("message", "value_received", message::value_received)?;

    linker.func_wrap("network", "base_fee", network::base_fee)?;
    linker.func_wrap("network", "version", network::version)?;
    linker.func_wrap("network", "epoch", network::epoch)?;

    linker.func_wrap("actor", "resolve_address", actor::resolve_address)?;
    linker.func_wrap("actor", "get_actor_code_cid", actor::get_actor_code_cid)?;
    linker.func_wrap("actor", "new_actor_address", actor::new_actor_address)?;
    linker.func_wrap("actor", "create_actor", actor::create_actor)?;

    linker.func_wrap("crypto", "verify_signature", crypto::verify_signature)?;
    linker.func_wrap("crypto", "hash_blake2b", crypto::hash_blake2b)?;
    linker.func_wrap("crypto", "verify_seal", crypto::verify_seal)?;
    linker.func_wrap("crypto", "verify_post", crypto::verify_post)?;
    linker.func_wrap(
        "crypto",
        "compute_unsealed_sector_cid",
        crypto::compute_unsealed_sector_cid,
    )?;
    linker.func_wrap(
        "crypto",
        "verify_consensus_fault",
        crypto::verify_consensus_fault,
    )?;
    linker.func_wrap(
        "crypto",
        "verify_aggregate_seals",
        crypto::verify_aggregate_seals,
    )?;
    // TODO implement
    // linker.func_wrap("crypto", "batch_verify_seals", crypto::batch_verify_seals)?;

    linker.func_wrap("rand", "get_chain_randomness", rand::get_chain_randomness)?;
    linker.func_wrap("rand", "get_beacon_randomness", rand::get_beacon_randomness)?;

    linker.func_wrap("gas", "charge_gas", gas::charge_gas)?;

    Ok(())
}
