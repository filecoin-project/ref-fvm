use fvm_shared::actor::builtin::Type;
use fvm_shared::sys;
use num_traits::FromPrimitive;

use super::Context;
use crate::kernel::{ClassifyResult, Result};
use crate::{syscall_error, Kernel};

pub fn resolve_address(
    context: Context<'_, impl Kernel>,
    addr_off: u32, // Address
    addr_len: u32,
) -> Result<sys::out::actor::ResolveAddress> {
    let addr = context.memory.read_address(addr_off, addr_len)?;
    let (resolved, value) = match context.kernel.resolve_address(&addr)? {
        Some(id) => (0, id),
        None => (-1, 0),
    };
    Ok(sys::out::actor::ResolveAddress { resolved, value })
}

pub fn get_actor_code_cid(
    context: Context<'_, impl Kernel>,
    addr_off: u32, // Address
    addr_len: u32,
    obuf_off: u32, // Cid
    obuf_len: u32,
) -> Result<i32> {
    let addr = context.memory.read_address(addr_off, addr_len)?;
    match context.kernel.get_actor_code_cid(&addr)? {
        Some(typ) => {
            let obuf = context.memory.try_slice_mut(obuf_off, obuf_len)?;
            // TODO: This isn't always an illegal argument error, only when the buffer is too small.
            typ.write_bytes(obuf).or_illegal_argument()?;
            Ok(0)
        }
        None => Ok(-1),
    }
}

/// Generates a new actor address, and writes it into the supplied output buffer.
///
/// The output buffer must be at least 21 bytes long, which is the length of a
/// class 2 address (protocol-generated actor address). This will change in the
/// future when we introduce class 4 addresses to accommodate larger hashes.
///
/// TODO this method will be merged with create_actor in the near future.
pub fn new_actor_address(
    context: Context<'_, impl Kernel>,
    obuf_off: u32, // Address (out)
    obuf_len: u32,
) -> Result<u32> {
    if obuf_len < 21 {
        return Err(
            syscall_error!(IllegalArgument; "output buffer must have a minimum capacity of 21 bytes").into(),
        );
    }

    let addr = context.kernel.new_actor_address()?;
    let bytes = addr.to_bytes();

    let len = bytes.len();
    if len > obuf_len as usize {
        return Err(syscall_error!(IllegalArgument;
            "insufficient output buffer capacity; {} (new address) > {} (buffer capacity)",
            len, obuf_len
        )
        .into());
    }

    let obuf = context.memory.try_slice_mut(obuf_off, obuf_len)?;
    obuf[..len].copy_from_slice(bytes.as_slice());
    Ok(len as u32)
}

pub fn create_actor(
    context: Context<'_, impl Kernel>,
    actor_id: u64, // Address
    typ_off: u32,  // Cid
) -> Result<()> {
    let typ = context.memory.read_cid(typ_off)?;
    context.kernel.create_actor(typ, actor_id)
}

pub fn resolve_builtin_actor_type(
    context: Context<'_, impl Kernel>,
    code_cid_off: u32, // Cid
) -> Result<i32> {
    let cid = context.memory.read_cid(code_cid_off)?;
    let result = context.kernel.resolve_builtin_actor_type(&cid);
    Ok(result.map(|v| v as i32).unwrap_or(0))
}

pub fn get_code_cid_for_type(
    context: Context<'_, impl Kernel>,
    typ: i32,
    obuf_off: u32, // Cid
    obuf_len: u32,
) -> Result<i32> {
    let typ: Type = FromPrimitive::from_i32(typ)
        .ok_or_else(|| syscall_error!(IllegalArgument; "invalid actor type"))?;
    let cid = context
        .kernel
        .get_code_cid_for_type(typ)
        .or_illegal_argument()?;
    let len = {
        let obuf = context.memory.try_slice_mut(obuf_off, obuf_len)?;
        let bytes = cid.to_bytes();
        let len = bytes.len();
        if len > obuf_len as usize {
            return Err(syscall_error!(IllegalArgument; "insufficient output buffer capacity; {} > {}", len, obuf_len)
                .into());
        }
        obuf[..len].copy_from_slice(bytes.as_slice());
        len
    };
    Ok(len as i32)
}
