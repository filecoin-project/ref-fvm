use anyhow::Context as _;

use crate::{
    kernel::{ClassifyResult, Result},
    Kernel,
};

use super::Memory;

pub fn open(kernel: &mut impl Kernel, memory: &mut [u8], cid: u32) -> Result<(u32, u64, u32)> {
    let cid = memory.read_cid(cid)?;
    let (id, stat) = kernel.block_open(&cid)?;
    Ok((id, stat.codec, stat.size))
}

pub fn create(
    kernel: &mut impl Kernel,
    memory: &mut [u8],
    codec: u64,
    data_off: u32,
    data_len: u32,
) -> Result<u32> {
    let data = memory.try_slice(data_off, data_len)?;
    kernel.block_create(codec, data)
}

pub fn cid(
    kernel: &mut impl Kernel,
    memory: &mut [u8],
    id: u32,
    hash_fun: u64,
    hash_len: u32,
    cid_off: u32,
    cid_len: u32,
) -> Result<u32> {
    let cid = kernel.block_link(id, hash_fun, hash_len)?;

    let size = super::encoded_cid_size(&cid);
    if size > cid_len {
        return Ok(size);
    }

    let mut out_slice = memory.try_slice_mut(cid_off, cid_len)?;

    cid.write_bytes(&mut out_slice)
        .context("failed to encode cid")
        .or_fatal()?;
    Ok(size)
}

pub fn read(
    kernel: &mut impl Kernel,
    memory: &mut [u8],
    id: u32,
    offset: u32,
    obuf_off: u32,
    obuf_len: u32,
) -> Result<u32> {
    let data = memory.try_slice_mut(obuf_off, obuf_len)?;
    kernel.block_read(id, offset, data)
}

pub fn stat(kernel: &mut impl Kernel, id: u32) -> Result<(u64, u32)> {
    kernel.block_stat(id).map(|stat| (stat.codec, stat.size))
}
