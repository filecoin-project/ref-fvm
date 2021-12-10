use crate::syscalls::context::Context;
use crate::Kernel;
use wasmtime::{Caller, Trap};

pub fn curr_epoch(caller: Caller<'_, impl Kernel>) -> Result<u64, Trap> {
    let ctx = Context::new(caller);

    Ok(ctx.data().network_curr_epoch() as u64)
}

pub fn version(caller: Caller<'_, impl Kernel>) -> Result<u32, Trap> {
    let ctx = Context::new(caller);

    Ok(ctx.data().network_version().into())
}

pub fn base_fee(caller: Caller<'_, impl Kernel>, into_off: u32) -> Result<(u64, u64), Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    ctx.data().network_base_fee()
    Ok(ctx.data().network_version().into())
}
