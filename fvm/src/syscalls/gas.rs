use std::str;

use crate::kernel::{ClassifyResult, Result};
use crate::Kernel;

use super::Context;

pub fn charge_gas(
    context: Context<'_, impl Kernel>,
    name_off: u32,
    name_len: u32,
    compute: i64,
) -> Result<()> {
    let name =
        str::from_utf8(context.memory.try_slice(name_off, name_len)?).or_illegal_argument()?;
    context.kernel.charge_gas(name, compute)
}
