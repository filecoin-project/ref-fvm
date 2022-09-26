use super::Context;
use crate::kernel::{Kernel, Result};

pub fn timestamp(_context: Context<'_, impl Kernel>) -> Result<u64> {
    todo!()
}

pub fn blockhash(
    _context: Context<'_, impl Kernel>,
    _block: u32,
    _ret_off: u32,
    _ret_len: u32,
) -> Result<u32> {
    todo!()
}

pub fn gas_limit(_context: Context<'_, impl Kernel>) -> Result<u64> {
    todo!()
}

pub fn gas_price(_context: Context<'_, impl Kernel>) -> Result<u64> {
    todo!()
}
