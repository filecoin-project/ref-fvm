use super::Context;
use crate::kernel::{Kernel, Result};

pub fn tipset_timestamp(_context: Context<'_, impl Kernel>) -> Result<u64> {
    todo!()
}

pub fn tipset_cid(
    _context: Context<'_, impl Kernel>,
    _epoch: i64,
    _ret_off: u32,
    _ret_len: u32,
) -> Result<u32> {
    todo!()
}
