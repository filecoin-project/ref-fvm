use super::{Context, MAX_CID_LEN};
use crate::kernel::ExecutionError;
use crate::Kernel;
use wasmtime::{Caller, Trap};

pub fn root(mut caller: Caller<'_, impl Kernel>, obuf_off: u32) -> Result<(), Trap> {
    let (kernel, mut memory) = caller.kernel_and_memory()?;
    let obuf = memory.try_slice_mut(obuf_off, obuf_off + MAX_CID_LEN as u32)?;
    let cid = kernel.root();
    cid.write_bytes(&mut obuf[..MAX_CID_LEN])
        .map_err(ExecutionError::from)?;
    Ok(())
}

pub fn set_root(mut caller: Caller<'_, impl Kernel>, cid_off: u32) -> Result<(), Trap> {
    let (kernel, memory) = caller.kernel_and_memory()?;
    let cid = memory.read_cid(cid_off)?;
    kernel.set_root(cid)?;
    Ok(())
}

pub fn current_balance(mut caller: Caller<'_, impl Kernel>) -> Result<(u64, u64), Trap> {
    let balance = caller.kernel().current_balance()?;
    let mut iter = balance.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}

/// TODO it should be possible to consume an address without knowing its length a priori
pub fn self_destruct(
    mut caller: Caller<'_, impl Kernel>,
    addr_off: u32,
    addr_len: u32,
) -> Result<(), Trap> {
    let (kernel, memory) = caller.kernel_and_memory()?;
    let addr = memory.read_address(addr_off, addr_len)?;
    kernel.self_destruct(&addr)?;
    Ok(())
}
