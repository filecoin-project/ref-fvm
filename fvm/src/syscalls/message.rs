use crate::kernel::BlockId;
use crate::syscalls::context::Context;
use crate::Kernel;
use wasmtime::{Caller, Trap};

pub fn caller(caller: Caller<'_, impl Kernel>) -> Result<u64, Trap> {
    Ok(Context::new(caller).data().msg_caller())
}

pub fn receiver(caller: Caller<'_, impl Kernel>) -> Result<u64, Trap> {
    Ok(Context::new(caller).data().msg_receiver())
}

pub fn method_number(caller: Caller<'_, impl Kernel>) -> Result<u64, Trap> {
    Ok(Context::new(caller).data().msg_method_number())
}

pub fn value_received(caller: Caller<'_, impl Kernel>) -> Result<(u64, u64), Trap> {
    let ctx = Context::new(caller);
    let value = ctx.data().msg_value_received();
    let mut iter = value.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}
