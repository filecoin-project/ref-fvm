use std::str;

use super::Context;
use crate::gas::Gas;
use crate::kernel::{ClassifyResult, Result};
use crate::Kernel;

pub fn charge_gas(
    context: Context<'_, impl Kernel>,
    name_off: u32,
    name_len: u32,
    compute: i64,
) -> Result<()> {
    let name =
        str::from_utf8(context.memory.try_slice(name_off, name_len)?).or_illegal_argument()?;
    // Gas charges from actors are always in full gas units. We use milligas internally, so convert here.
    context.kernel.charge_gas(name, Gas::new(compute))
}
