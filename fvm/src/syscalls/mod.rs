use wasmtime::Linker;

use crate::Kernel;
pub(crate) mod error;

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

use self::error::BindSyscall;

/// The maximum supported CID size. (SPEC_AUDIT)
pub const MAX_CID_LEN: usize = 100;

// Binds the syscall handlers so they can handle invocations
// from the actor code.
//
// TODO try to fix the static lifetime here. I want to tell the compiler that
//  the Kernel will live as long as the Machine and the Linker.
pub fn bind_syscalls<K: Kernel + 'static>(linker: &mut Linker<K>) -> anyhow::Result<()> {
    /*
    macro_rules! bind {
        ($module:ident :: $func:ident) => {
            linker.func_wrap(stringify!($module), stringify!($func), || $module::$func
        }

    }
    */
    linker.bind("ipld", "get_root", ipld::get_root)?;
    linker.bind("ipld", "set_root", ipld::set_root)?;
    linker.bind("ipld", "open", ipld::open)?;
    linker.bind("ipld", "create", ipld::create)?;
    linker.bind("ipld", "read", ipld::read)?;
    linker.bind("ipld", "stat", ipld::stat)?;
    linker.bind("ipld", "cid", ipld::cid)?;

    linker.bind(
        "validation",
        "accept_any",
        validation::validate_immediate_caller_accept_any,
    )?;
    linker.bind(
        "validation",
        "accept_addrs",
        validation::validate_immediate_caller_addr_one_of,
    )?;
    linker.bind(
        "validation",
        "accept_types",
        validation::validate_immediate_caller_type_one_of,
    )?;

    linker.bind("self", "root", sself::root)?;
    linker.bind("self", "set_root", sself::set_root)?;
    linker.bind("self", "current_balance", sself::current_balance)?;
    linker.bind("self", "self_destruct", sself::self_destruct)?;

    linker.bind("message", "caller", message::caller)?;
    linker.bind("message", "receiver", message::receiver)?;
    linker.bind("message", "method_number", message::method_number)?;
    linker.bind("message", "value_received", message::value_received)?;

    linker.bind("network", "base_fee", network::base_fee)?;
    linker.bind("network", "version", network::version)?;
    linker.bind("network", "epoch", network::epoch)?;

    linker.bind("actor", "resolve_address", actor::resolve_address)?;
    linker.bind("actor", "get_actor_code_cid", actor::get_actor_code_cid)?;
    linker.bind("actor", "new_actor_address", actor::new_actor_address)?;
    linker.bind("actor", "create_actor", actor::create_actor)?;

    linker.bind("crypto", "verify_signature", crypto::verify_signature)?;
    linker.bind("crypto", "hash_blake2b", crypto::hash_blake2b)?;
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
    // TODO implement
    // linker.bind("crypto", "batch_verify_seals", crypto::batch_verify_seals)?;

    linker.bind("rand", "get_chain_randomness", rand::get_chain_randomness)?;
    linker.bind("rand", "get_beacon_randomness", rand::get_beacon_randomness)?;

    linker.bind("gas", "charge_gas", gas::charge_gas)?;

    Ok(())
}
