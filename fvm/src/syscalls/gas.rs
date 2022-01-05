use crate::kernel::{ClassifyResult, Result};
use crate::syscalls::Memory;
use crate::Kernel;
use std::str;

pub fn charge_gas(
    kernel: &mut impl Kernel,
    memory: &mut [u8],
    name_off: u32,
    name_len: u32,
    compute: i64,
) -> Result<()> {
    let name = str::from_utf8(memory.try_slice(name_off, name_len)?).or_illegal_argument()?;
    kernel.charge_gas(name, compute)
}
