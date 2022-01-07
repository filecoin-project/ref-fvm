use super::{Context, MAX_CID_LEN};
use crate::kernel::{ClassifyResult, ExecutionError, Kernel, Result};
use anyhow::{anyhow, Context as _};

pub fn root(context: Context<'_, impl Kernel>, obuf_off: u32) -> Result<()> {
    let root = context.kernel.root();
    let size = super::encoded_cid_size(&root);
    if size > MAX_CID_LEN as u32 {
        return Err(ExecutionError::Fatal(anyhow!(
            "root CID length larger than CID length allowed by environment: {} > {}",
            size,
            MAX_CID_LEN
        )));
    }

    let obuf = context
        .memory
        .try_slice_mut(obuf_off, obuf_off + MAX_CID_LEN as u32)?;
    root.write_bytes(&mut obuf[..MAX_CID_LEN])
        .context("failed to write cid root")
        .or_fatal()?;

    Ok(())
}

pub fn set_root(context: Context<'_, impl Kernel>, cid_off: u32) -> Result<()> {
    let cid = context.memory.read_cid(cid_off)?;
    context.kernel.set_root(cid)?;
    Ok(())
}

pub fn current_balance(context: Context<'_, impl Kernel>) -> Result<(u64, u64)> {
    let balance = context.kernel.current_balance()?;
    let mut iter = balance.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}

/// TODO it should be possible to consume an address without knowing its length a priori
pub fn self_destruct(
    context: Context<'_, impl Kernel>,
    addr_off: u32,
    addr_len: u32,
) -> Result<()> {
    let addr = context.memory.read_address(addr_off, addr_len)?;
    context.kernel.self_destruct(&addr)?;
    Ok(())
}
