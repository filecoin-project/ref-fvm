use crate::kernel::{ClassifyResult, Result};
use crate::syscalls::context::Context;
use crate::Kernel;
use std::str;
use wasmtime::Caller;

pub fn charge_gas(
    caller: &mut Caller<'_, impl Kernel>,
    name_off: u32,
    name_len: u32,
    compute: i64,
) -> Result<()> {
    let (k, mem) = caller.kernel_and_memory()?;
    let name = str::from_utf8(mem.try_slice(name_off, name_len)?).or_illegal_argument()?;
    k.charge_gas(name, compute)
}
