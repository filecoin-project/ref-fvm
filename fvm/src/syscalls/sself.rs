use super::Context;
use crate::kernel::{ClassifyResult, Kernel, Result};
use anyhow::Context as _;
use fvm_shared::sys;

/// Returns the root CID of the actor's state by writing it in the specified buffer.
///
/// The returned u32 represents the _actual_ length of the CID. If the supplied
/// buffer is smaller, no value will have been written. The caller must retry
/// with a larger buffer.
pub fn root(context: Context<'_, impl Kernel>, obuf_off: u32, obuf_len: u32) -> Result<u32> {
    let root = context.kernel.root();
    let size = super::encoded_cid_size(&root);

    if size <= obuf_len {
        // Only write the CID if there's sufficient capacity.
        let mut obuf = context.memory.try_slice_mut(obuf_off, size)?;

        root.write_bytes(&mut obuf)
            .context("failed to write cid root")
            .or_fatal()?;
    }

    Ok(size)
}

pub fn set_root(context: Context<'_, impl Kernel>, cid_off: u32) -> Result<()> {
    let cid = context.memory.read_cid(cid_off)?;
    context.kernel.set_root(cid)?;
    Ok(())
}

pub fn current_balance(context: Context<'_, impl Kernel>) -> Result<sys::TokenAmount> {
    let balance = context.kernel.current_balance()?;
    let mut iter = balance.iter_u64_digits();
    Ok(sys::TokenAmount {
        lo: iter.next().unwrap(),
        hi: iter.next().unwrap_or(0),
    })
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
