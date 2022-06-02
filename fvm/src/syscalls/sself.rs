use anyhow::Context as _;
use fvm_shared::sys;

use super::Context;
use crate::kernel::{ClassifyResult, Kernel, Result};

/// Returns the root CID of the actor's state by writing it in the specified buffer.
///
/// The returned u32 represents the _actual_ length of the CID. If the supplied
/// buffer is smaller, no value will have been written. The caller must retry
/// with a larger buffer.
pub fn root(context: Context<'_, impl Kernel>, obuf_off: u32, obuf_len: u32) -> Result<u32> {
    context.memory.check_bounds(obuf_off, obuf_len)?;

    let root = context.kernel.root()?;

    context.memory.write_cid(&root, obuf_off, obuf_len)
}

pub fn set_root(context: Context<'_, impl Kernel>, cid_off: u32) -> Result<()> {
    let cid = context.memory.read_cid(cid_off)?;
    context.kernel.set_root(cid)?;
    Ok(())
}

pub fn current_balance(context: Context<'_, impl Kernel>) -> Result<sys::TokenAmount> {
    let balance = context.kernel.current_balance()?;
    balance
        .try_into()
        .context("balance exceeds u128")
        .or_fatal()
}

pub fn self_destruct(
    context: Context<'_, impl Kernel>,
    addr_off: u32,
    addr_len: u32,
) -> Result<()> {
    let addr = context.memory.read_address(addr_off, addr_len)?;
    context.kernel.self_destruct(&addr)?;
    Ok(())
}
