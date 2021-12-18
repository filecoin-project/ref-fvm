use crate::kernel::{ExecutionError, SyscallError};
use crate::syscalls::context::Context;
use crate::Kernel;
use std::str;
use wasmtime::{Caller, Trap};

pub fn charge_gas(
    mut caller: Caller<'_, impl Kernel>,
    name_off: u32,
    name_len: u32,
    compute: i64,
) -> Result<(), Trap> {
    let (k, mem) = caller.kernel_and_memory()?;
    let name = mem
        .try_slice(name_off, name_len)
        .map(|bytes| str::from_utf8(bytes).map_err(|e| SyscallError(e.to_string(), None)))?
        .map_err(ExecutionError::from)
        .map_err(Trap::from)?;
    k.charge_gas(name, compute).map_err(Trap::from)
}
