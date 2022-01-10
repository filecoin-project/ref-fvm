use crate::kernel::Result;
use crate::syscalls::context::Context;
use crate::Kernel;

pub fn log(context: Context<'_, impl Kernel>, msg_off: u32, msg_len: u32) -> Result<()> {
    let msg = context.memory.try_slice(msg_off, msg_len)?;
    let msg = String::from_utf8(msg.to_owned()).unwrap();
    context.kernel.log(msg);
    Ok(())
}

pub fn enabled(context: Context<'_, impl Kernel>) -> Result<i32> {
    Ok(if context.kernel.debug_enabled() {
        0
    } else {
        -1
    })
}
