use anyhow::Context as _;
use cid::{self, Cid};

use crate::{
    kernel::{ClassifyResult, Result},
    Kernel,
};

use super::Memory;

// Computes the encoded size of a varint.
// TODO: move this to the varint crate.
fn uvarint_size(num: u64) -> u32 {
    let bits = u64::BITS - num.leading_zeros();
    (bits / 7 + (bits % 7 > 0) as u32).min(1) as u32
}

/// Returns the size cid would be, once encoded.
// TODO: move this to the cid/multihash crates.
fn encoded_cid_size(k: &Cid) -> u32 {
    let mh = k.hash();
    let mh_size = uvarint_size(mh.code()) + uvarint_size(mh.size() as u64) + mh.size() as u32;
    match k.version() {
        cid::Version::V0 => mh_size,
        cid::Version::V1 => mh_size + uvarint_size(k.codec()) + 1,
    }
}

pub fn get_root(
    kernel: &mut impl Kernel,
    memory: &mut [u8],
    cid_off: u32,
    cid_len: u32,
) -> Result<u32> {
    let root = kernel.root();
    let size = encoded_cid_size(&root);
    if size > cid_len {
        return Ok(size);
    }

    let mut out_slice = memory.try_slice_mut(cid_off, cid_len)?;

    root.write_bytes(&mut out_slice)
        .context("failed to write cid root")
        .or_fatal()?;

    Ok(size)
}

pub fn set_root(kernel: &mut impl Kernel, memory: &mut [u8], cid: u32) -> Result<()> {
    let cid = memory.read_cid(cid)?;
    kernel.set_root(cid)?;
    Ok(())
}

pub fn open(kernel: &mut impl Kernel, memory: &mut [u8], cid: u32) -> Result<u32> {
    let cid = memory.read_cid(cid)?;
    kernel.block_open(&cid)
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

    let size = encoded_cid_size(&cid);
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
