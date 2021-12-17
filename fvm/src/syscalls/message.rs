use crate::Kernel;
use wasmtime::{Caller, Trap};

use super::Context;

pub fn caller(mut caller: Caller<'_, impl Kernel>) -> Result<u64, Trap> {
    Ok(caller.kernel().msg_caller())
}

pub fn receiver(mut caller: Caller<'_, impl Kernel>) -> Result<u64, Trap> {
    Ok(caller.kernel().msg_receiver())
}

pub fn method_number(mut caller: Caller<'_, impl Kernel>) -> Result<u64, Trap> {
    Ok(caller.kernel().msg_method_number())
}

pub fn value_received(mut caller: Caller<'_, impl Kernel>) -> Result<(u64, u64), Trap> {
    let kernel = caller.kernel();
    let value = kernel.msg_value_received();
    let mut iter = value.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}
