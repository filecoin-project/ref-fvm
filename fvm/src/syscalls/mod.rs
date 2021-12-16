use wasmtime::Linker;

use crate::{kernel::Result, Kernel};

mod context;
mod ipld;
mod message;
mod network;
mod sself;
mod typestate;
mod validation;

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

    Ok(())
}
