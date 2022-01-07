use crate::kernel::{Kernel, Result};

use super::Context;

pub fn caller(context: Context<'_, impl Kernel>) -> Result<u64> {
    Ok(context.kernel.msg_caller())
}

pub fn receiver(context: Context<'_, impl Kernel>) -> Result<u64> {
    Ok(context.kernel.msg_receiver())
}

pub fn method_number(context: Context<'_, impl Kernel>) -> Result<u64> {
    Ok(context.kernel.msg_method_number())
}

pub fn value_received(context: Context<'_, impl Kernel>) -> Result<(u64, u64)> {
    let value = context.kernel.msg_value_received();
    let mut iter = value.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}
