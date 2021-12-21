use crate::kernel::{Kernel, Result};
use wasmtime::Caller;

use super::Context;

pub fn caller(caller: &mut Caller<'_, impl Kernel>) -> Result<u64> {
    Ok(caller.kernel().msg_caller())
}

pub fn receiver(caller: &mut Caller<'_, impl Kernel>) -> Result<u64> {
    Ok(caller.kernel().msg_receiver())
}

pub fn method_number(caller: &mut Caller<'_, impl Kernel>) -> Result<u64> {
    Ok(caller.kernel().msg_method_number())
}

pub fn value_received(caller: &mut Caller<'_, impl Kernel>) -> Result<(u64, u64)> {
    let kernel = caller.kernel();
    let value = kernel.msg_value_received();
    let mut iter = value.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}
