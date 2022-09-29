use super::Context;
use crate::kernel::{Kernel, Result};

pub fn tipset_timestamp(context: Context<'_, impl Kernel>) -> Result<u64> {
    Ok(context.kernel.tipset_timestamp())
}

pub fn tipset_cid(
    context: Context<'_, impl Kernel>,
    epoch: i64,
    obuf_off: u32,
    obuf_len: u32,
) -> Result<u32> {
    // We always check arguments _first_, before we do anything else.
    context.memory.check_bounds(obuf_off, obuf_len)?;

    if let Some(cid) = context.kernel.tipset_cid(epoch)? {
        context.memory.write_cid(&cid, obuf_off, obuf_len)
    } else {
        Ok(0)
    }
}
