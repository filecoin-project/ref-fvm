use crate::{kernel::ExecutionError, Kernel};
use cid::Cid;
use fvm_shared::address::Address;
use wasmtime::{Caller, Trap};

use super::{get_kernel, get_kernel_and_memory};

pub fn validate_immediate_caller_accept_any(
    mut caller: Caller<'_, impl Kernel>,
) -> Result<(), Trap> {
    get_kernel(&mut caller).validate_immediate_caller_accept_any()?;
    Ok(())
}

pub fn validate_immediate_caller_addr_one_of(
    mut caller: Caller<'_, impl Kernel>,
    addrs_offset: u32,
    addrs_len: u32,
) -> Result<(), Trap> {
    let (kernel, memory) = get_kernel_and_memory(&mut caller)?;
    let bytes = memory.try_slice(addrs_offset, addrs_len)?;
    // TODO sugar for enveloping unboxed errors into traps.
    let addrs: Vec<Address> =
        fvm_shared::encoding::from_slice(bytes).map_err(ExecutionError::from)?;
    kernel.validate_immediate_caller_addr_one_of(addrs.as_slice())?;

    Ok(())
}

pub fn validate_immediate_caller_type_one_of(
    mut caller: Caller<'_, impl Kernel>,
    cids_offset: u32,
    cids_len: u32,
) -> Result<(), Trap> {
    let (kernel, memory) = get_kernel_and_memory(&mut caller)?;
    let bytes = memory.try_slice(cids_offset, cids_len)?;
    let cids: Vec<Cid> = fvm_shared::encoding::from_slice(bytes).map_err(ExecutionError::from)?;

    kernel.validate_immediate_caller_type_one_of(cids.as_slice())?;
    Ok(())
}
