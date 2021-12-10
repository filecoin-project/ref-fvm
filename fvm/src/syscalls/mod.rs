use anyhow::Result;
use wasmtime::Linker;

use crate::Kernel;

mod context;
mod ipld;
mod network;
mod typestate;

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
    Ok(())
}
