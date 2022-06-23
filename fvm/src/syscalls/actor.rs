use anyhow::anyhow;
use fvm_shared::actor::builtin::Type;
use num_traits::FromPrimitive;

use super::Context;
use crate::kernel::{ClassifyResult, Result};
use crate::{syscall_error, Kernel};

pub fn resolve_address(
    context: Context<'_, impl Kernel>,
    addr_off: u32, // Address
    addr_len: u32,
) -> Result<u64> {
    let addr = context.memory.read_address(addr_off, addr_len)?;
    let actor_id = context
        .kernel
        .resolve_address(&addr)?
        .ok_or_else(|| syscall_error!(NotFound; "actor not found"))?;
    Ok(actor_id)
}

pub fn get_actor_code_cid(
    context: Context<'_, impl Kernel>,
    actor_id: u64,
    obuf_off: u32, // Cid
    obuf_len: u32,
) -> Result<u32> {
    // We always check arguments _first_, before we do anything else.
    context.memory.check_bounds(obuf_off, obuf_len)?;

    let typ = context
        .kernel
        .get_actor_code_cid(actor_id)?
        .ok_or_else(|| syscall_error!(NotFound; "target actor not found"))?;

    context.memory.write_cid(&typ, obuf_off, obuf_len)
}

/// Generates a new actor address, and writes it into the supplied output buffer.
///
/// The output buffer must be at least 21 bytes long, which is the length of a
/// class 2 address (protocol-generated actor address). This will change in the
/// future when we introduce class 4 addresses to accommodate larger hashes.
///
/// TODO(M2): this method will be merged with create_actor.
pub fn new_actor_address(
    context: Context<'_, impl Kernel>,
    obuf_off: u32, // Address (out)
    obuf_len: u32,
) -> Result<u32> {
    // Check bounds first.
    let obuf = context.memory.try_slice_mut(obuf_off, obuf_len)?;

    // Then make sure we can actually put the return result somewhere before we do anything else.
    const EXPECTED_LEN: u32 = fvm_shared::address::PAYLOAD_HASH_LEN as u32 + 1;
    if obuf_len < EXPECTED_LEN {
        return Err(
            syscall_error!(BufferTooSmall; "output buffer must have a minimum capacity of 21 bytes").into(),
        );
    }

    // Create the address.
    let addr = context.kernel.new_actor_address()?;

    // And return it.
    let bytes = addr.to_bytes();
    let len = bytes.len();
    // Sanity check the length, and fail the entire message if something went wrong. This should
    // never happen.
    if len > obuf_len as usize {
        // This is _fatal_ because it means we've already allocated an ID for the address, but can't
        // use it.
        return Err(anyhow!("created {} byte actor address", len)).or_fatal();
    }

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

pub fn get_builtin_actor_type(
    context: Context<'_, impl Kernel>,
    code_cid_off: u32, // Cid
) -> Result<i32> {
    let cid = context.memory.read_cid(code_cid_off)?;
    let result = context.kernel.get_builtin_actor_type(&cid);
    Ok(result.map(|v| v as i32).unwrap_or(0))
}

pub fn get_code_cid_for_type(
    context: Context<'_, impl Kernel>,
    typ: i32,
    obuf_off: u32, // Cid
    obuf_len: u32,
) -> Result<u32> {
    // Check params in-order.
    let typ: Type = FromPrimitive::from_i32(typ)
        .ok_or_else(|| syscall_error!(IllegalArgument; "invalid actor type"))?;
    context.memory.check_bounds(obuf_off, obuf_len)?;

    let k = context.kernel.get_code_cid_for_type(typ)?;
    context.memory.write_cid(&k, obuf_off, obuf_len)
}

#[cfg(feature = "m2-native")]
pub fn install_actor(
    context: Context<'_, impl Kernel>,
    typ_off: u32, // Cid
) -> Result<()> {
    let typ = context.memory.read_cid(typ_off)?;
    context.kernel.install_actor(typ)
}
