// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::{anyhow, Context as _};
use fvm_shared::{sys, ActorID};

use super::Context;
use crate::kernel::{ClassifyResult, Result};
use crate::{syscall_error, Kernel};

pub fn resolve_address(
    context: Context<'_, impl Kernel>,
    addr_off: u32, // Address
    addr_len: u32,
) -> Result<u64> {
    let addr = context.memory.read_address(addr_off, addr_len)?;
    let actor_id = context.kernel.resolve_address(&addr)?;
    Ok(actor_id)
}

pub fn lookup_delegated_address(
    context: Context<'_, impl Kernel>,
    actor_id: ActorID,
    obuf_off: u32,
    obuf_len: u32,
) -> Result<u32> {
    let obuf = context.memory.try_slice_mut(obuf_off, obuf_len)?;
    match context.kernel.lookup_delegated_address(actor_id)? {
        Some(address) => {
            let address = address.to_bytes();
            obuf.get_mut(..address.len())
                .ok_or_else(
                    || syscall_error!(BufferTooSmall; "address output buffer is too small"),
                )?
                .copy_from_slice(&address);
            Ok(address.len() as u32)
        }
        None => Ok(0),
    }
}

pub fn get_actor_code_cid(
    context: Context<'_, impl Kernel>,
    actor_id: u64,
    obuf_off: u32, // Cid
    obuf_len: u32,
) -> Result<u32> {
    // We always check arguments _first_, before we do anything else.
    context.memory.check_bounds(obuf_off, obuf_len)?;

    let typ = context.kernel.get_actor_code_cid(actor_id)?;

    context.memory.write_cid(&typ, obuf_off, obuf_len)
}

/// Generates a new actor address, and writes it into the supplied output buffer.
///
/// The output buffer must be at least 21 bytes long, which is the length of a class 2 address
/// (protocol-generated actor address).
pub fn next_actor_address(
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
    let addr = context.kernel.next_actor_address()?;

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
    actor_id: u64, // ID
    typ_off: u32,  // Cid
    delegated_addr_off: u32,
    delegated_addr_len: u32,
) -> Result<()> {
    let typ = context.memory.read_cid(typ_off)?;
    let addr = (delegated_addr_len > 0)
        .then(|| {
            context
                .memory
                .read_address(delegated_addr_off, delegated_addr_len)
        })
        .transpose()?;

    context.kernel.create_actor(typ, actor_id, addr)
}

pub fn get_builtin_actor_type(
    context: Context<'_, impl Kernel>,
    code_cid_off: u32, // Cid
) -> Result<i32> {
    let cid = context.memory.read_cid(code_cid_off)?;
    Ok(context.kernel.get_builtin_actor_type(&cid)? as i32)
}

pub fn get_code_cid_for_type(
    context: Context<'_, impl Kernel>,
    typ: i32,
    obuf_off: u32, // Cid
    obuf_len: u32,
) -> Result<u32> {
    context.memory.check_bounds(obuf_off, obuf_len)?;

    let k = context.kernel.get_code_cid_for_type(typ as u32)?;
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

pub fn balance_of(context: Context<'_, impl Kernel>, actor_id: u64) -> Result<sys::TokenAmount> {
    let balance = context.kernel.balance_of(actor_id)?;
    balance
        .try_into()
        .context("balance exceeds u128 limit")
        .or_fatal()
}
