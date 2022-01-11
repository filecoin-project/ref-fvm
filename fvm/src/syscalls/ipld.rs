use anyhow::Context as _;
use fvm_shared::sys;

use super::Context;
use crate::kernel::{ClassifyResult, Result};
use crate::Kernel;

pub fn open(context: Context<'_, impl Kernel>, cid: u32) -> Result<sys::out::ipld::IpldOpen> {
    let cid = context.memory.read_cid(cid)?;
    let (id, stat) = context.kernel.block_open(&cid)?;
    Ok(sys::out::ipld::IpldOpen {
        id,
        codec: stat.codec,
        size: stat.size,
    })
}

pub fn create(
    context: Context<'_, impl Kernel>,
    codec: u64,
    data_off: u32,
    data_len: u32,
) -> Result<u32> {
    let data = context.memory.try_slice(data_off, data_len)?;
    context.kernel.block_create(codec, data)
}

pub fn cid(
    context: Context<'_, impl Kernel>,
    id: u32,
    hash_fun: u64,
    hash_len: u32,
    cid_off: u32,
    cid_len: u32,
) -> Result<u32> {
    let cid = context.kernel.block_link(id, hash_fun, hash_len)?;

    let size = super::encoded_cid_size(&cid);
    if size > cid_len {
        return Ok(size);
    }

    let mut out_slice = context.memory.try_slice_mut(cid_off, cid_len)?;

    cid.write_bytes(&mut out_slice)
        .context("failed to encode cid")
        .or_fatal()?;
    Ok(size)
}

pub fn read(
    context: Context<'_, impl Kernel>,
    id: u32,
    offset: u32,
    obuf_off: u32,
    obuf_len: u32,
) -> Result<u32> {
    let data = context.memory.try_slice_mut(obuf_off, obuf_len)?;
    context.kernel.block_read(id, offset, data)
}

pub fn stat(context: Context<'_, impl Kernel>, id: u32) -> Result<sys::out::ipld::IpldStat> {
    context
        .kernel
        .block_stat(id)
        .map(|stat| sys::out::ipld::IpldStat {
            codec: stat.codec,
            size: stat.size,
        })
}
