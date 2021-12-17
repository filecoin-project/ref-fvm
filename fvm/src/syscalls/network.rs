use crate::Kernel;
use wasmtime::{Caller, Trap};

use super::Context;

pub fn epoch(mut caller: Caller<'_, impl Kernel>) -> Result<u64, Trap> {
    Ok(caller.kernel().network_epoch() as u64)
}

pub fn version(mut caller: Caller<'_, impl Kernel>) -> Result<u32, Trap> {
    Ok(caller.kernel().network_version() as u32)
}

/// Returns the base fee split as two u64 ordered in little endian.
pub fn base_fee(mut caller: Caller<'_, impl Kernel>) -> Result<(u64, u64), Trap> {
    let base_fee = caller.kernel().network_base_fee();
    let mut iter = base_fee.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}
