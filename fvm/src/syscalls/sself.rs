use super::{Memory, MAX_CID_LEN};
use crate::kernel::{ClassifyResult, Kernel, Result};

pub fn root(kernel: &mut impl Kernel, memory: &mut [u8], obuf_off: u32) -> Result<()> {
    let obuf = memory.try_slice_mut(obuf_off, obuf_off + MAX_CID_LEN as u32)?;
    let cid = kernel.root();
    cid.write_bytes(&mut obuf[..MAX_CID_LEN]).or_fatal()?;
    Ok(())
}

pub fn set_root(kernel: &mut impl Kernel, memory: &mut [u8], cid_off: u32) -> Result<()> {
    let cid = memory.read_cid(cid_off)?;
    kernel.set_root(cid)?;
    Ok(())
}

pub fn current_balance(kernel: &mut impl Kernel) -> Result<(u64, u64)> {
    let balance = kernel.current_balance()?;
    let mut iter = balance.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}

/// TODO it should be possible to consume an address without knowing its length a priori
pub fn self_destruct(
    kernel: &mut impl Kernel,
    memory: &mut [u8],
    addr_off: u32,
    addr_len: u32,
) -> Result<()> {
    let addr = memory.read_address(addr_off, addr_len)?;
    kernel.self_destruct(&addr)?;
    Ok(())
}
