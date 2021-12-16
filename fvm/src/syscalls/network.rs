use crate::syscalls::context::Context;
use crate::Kernel;
use wasmtime::{Caller, Trap};

pub fn epoch(caller: Caller<'_, impl Kernel>) -> Result<u64, Trap> {
    let ctx = Context::new(caller);

    Ok(ctx.data().network_epoch() as u64)
}

pub fn version(caller: Caller<'_, impl Kernel>) -> Result<u32, Trap> {
    let ctx = Context::new(caller);

    Ok(ctx.data().network_version() as u32)
}

/// Returns the base fee split as two u64 ordered in little endian.
pub fn base_fee(caller: Caller<'_, impl Kernel>) -> Result<(u64, u64), Trap> {
    let mut ctx = Context::new(caller);
    let base_fee = ctx.data().network_base_fee();
    let mut iter = base_fee.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}
