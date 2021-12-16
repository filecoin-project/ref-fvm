use super::MAX_CID_LEN;
use crate::Kernel;
use crate::{kernel::ExecutionError, syscalls::context::Context};
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use wasmtime::{Caller, Trap};

pub fn root(caller: Caller<'_, impl Kernel>, obuf_off: u32) -> Result<(), Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let (mut obuf, k) = ctx.try_slice_and_runtime(obuf_off, obuf_off + MAX_CID_LEN as u32)?;
    let cid = k.root();
    cid.write_bytes(&mut obuf[..MAX_CID_LEN])
        .map_err(ExecutionError::from)?;
    Ok(())
}

pub fn set_root(caller: Caller<'_, impl Kernel>, cid_off: u32) -> Result<(), Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let cid = ctx.read_cid(cid_off)?;
    ctx.data_mut().set_root(cid)?;
    Ok(())
}

pub fn current_balance(caller: Caller<'_, impl Kernel>) -> Result<(u64, u64), Trap> {
    let mut ctx = Context::new(caller);
    let balance = ctx.data().current_balance()?;
    let mut iter = balance.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}

/// TODO it should be possible to consume an address without knowing its length a priori
pub fn self_destruct(
    caller: Caller<'_, impl Kernel>,
    addr_off: u32,
    addr_len: u32,
) -> Result<(), Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let bytes = ctx.try_slice(addr_off, addr_len)?;
    let addr = Address::from_bytes(bytes).map_err(ExecutionError::from)?;
    ctx.data_mut().self_destruct(&addr)?;
    Ok(())
}
