use crate::kernel::{Kernel, Result};

use super::Context;

pub fn epoch(context: Context<'_, impl Kernel>) -> Result<u64> {
    Ok(context.kernel.network_epoch() as u64)
}

pub fn version(context: Context<'_, impl Kernel>) -> Result<u32> {
    Ok(context.kernel.network_version() as u32)
}

/// Returns the base fee split as two u64 ordered in little endian.
pub fn base_fee(context: Context<'_, impl Kernel>) -> Result<(u64, u64)> {
    let base_fee = context.kernel.network_base_fee();
    let mut iter = base_fee.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}

/// Returns the network circ supply split as two u64 ordered in little endian.
pub fn total_fil_circ_supply(context: Context<'_, impl Kernel>) -> Result<(u64, u64)> {
    let base_fee = context.kernel.total_fil_circ_supply()?;
    let mut iter = base_fee.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}
