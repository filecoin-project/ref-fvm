use anyhow::Result;
use std::error::Error;
use wasmtime::{self, Engine, Linker, Trap};

mod context;
mod ipld;
mod typestate;

/// Binds the syscall handlers so they can handle invocations
/// from the actor code.
pub fn bind_syscalls<R>(&mut linker: Linker<R>) -> Result<()> {
    linker.func_wrap("ipld", "get_root", ipld::get_root)?;
    linker.func_wrap("ipld", "set_root", ipld::set_root)?;
    linker.func_wrap("ipld", "open", ipld::open)?;
    linker.func_wrap("ipld", "create", ipld::create)?;
    linker.func_wrap("ipld", "read", ipld::read)?;
    linker.func_wrap("ipld", "stat", ipld::stat)?;
    linker.func_wrap("ipld", "cid", ipld::cid)?;
    Ok(())
}
