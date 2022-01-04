use crate::kernel::{Kernel, Result};

pub fn caller(kernel: &mut impl Kernel) -> Result<u64> {
    Ok(kernel.msg_caller())
}

pub fn receiver(kernel: &mut impl Kernel) -> Result<u64> {
    Ok(kernel.msg_receiver())
}

pub fn method_number(kernel: &mut impl Kernel) -> Result<u64> {
    Ok(kernel.msg_method_number())
}

pub fn value_received(kernel: &mut impl Kernel) -> Result<(u64, u64)> {
    let value = kernel.msg_value_received();
    let mut iter = value.iter_u64_digits();
    Ok((iter.next().unwrap(), iter.next().unwrap_or(0)))
}
