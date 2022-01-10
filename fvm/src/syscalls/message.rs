use crate::kernel::{Kernel, Result};
use fvm_shared::sys;

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

pub fn value_received(context: Context<'_, impl Kernel>) -> Result<sys::TokenAmount> {
    let value = context.kernel.msg_value_received();
    let mut iter = value.iter_u64_digits();
    Ok(sys::TokenAmount {
        lo: iter.next().unwrap(),
        hi: iter.next().unwrap_or(0),
    })
}
