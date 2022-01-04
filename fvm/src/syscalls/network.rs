use crate::kernel::{Kernel, Result};

pub fn epoch(kernel: &mut impl Kernel) -> Result<u64> {
    Ok(kernel.network_epoch() as u64)
}

pub fn version(kernel: &mut impl Kernel) -> Result<u32> {
    Ok(kernel.network_version() as u32)
}

/// Returns the base fee split as two u64 ordered in little endian.
pub fn base_fee(kernel: &mut impl Kernel) -> Result<(u64, u64)> {
    let base_fee = kernel.network_base_fee();
    let mut iter = base_fee.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}

/// Returns the network circ supply split as two u64 ordered in little endian.
pub fn total_fil_circ_supply(kernel: &mut impl Kernel) -> Result<(u64, u64)> {
    let base_fee = kernel.total_fil_circ_supply()?;
    let mut iter = base_fee.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}
