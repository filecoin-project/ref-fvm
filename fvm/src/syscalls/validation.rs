use crate::syscalls::context::Context;
use crate::Kernel;
use cid::Cid;
use fvm_shared::address::Address;
use wasmtime::{Caller, Trap};

pub fn validate_immediate_caller_accept_any(caller: Caller<'_, impl Kernel>) -> Result<(), Trap> {
    Context::new(caller)
        .data_mut()
        .validate_immediate_caller_accept_any()
        .map_err(|e| Trap::from(Box::from(e)))
}

pub fn validate_immediate_caller_addr_one_of(
    caller: Caller<'_, impl Kernel>,
    addrs_offset: u32,
    addrs_len: u32,
) -> Result<(), Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let bytes = ctx.try_slice(addrs_offset, addrs_len)?;
    // TODO sugar for enveloping unboxed errors into traps.
    let addrs: Vec<Address> =
        fvm_shared::encoding::from_slice(bytes).map_err(|e| Trap::from(Box::from(e)))?;
    ctx.data_mut()
        .validate_immediate_caller_addr_one_of(addrs.as_slice())
        .map_err(|e| Trap::from(Box::from(e)))
}

pub fn validate_immediate_caller_type_one_of(
    caller: Caller<'_, impl Kernel>,
    cids_offset: u32,
    cids_len: u32,
) -> Result<(), Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let bytes = ctx.try_slice(cids_offset, cids_len)?;
    let cids: Vec<Cid> =
        fvm_shared::encoding::from_slice(bytes).map_err(|e| Trap::from(Box::from(e)))?;
    ctx.data_mut()
        .validate_immediate_caller_type_one_of(cids.as_slice())
        .map_err(|e| Trap::from(Box::from(e)))
}
