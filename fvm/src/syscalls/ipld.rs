use cid::{self, Cid};
use wasmtime::{self, Caller, Trap};

use crate::Kernel;

use super::context::Context;

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

pub fn get_root(caller: Caller<'_, impl Kernel>, cid_off: u32, cid_len: u32) -> Result<u32, Trap> {
    let ctx = Context::new(caller);

    let root = ctx.data().root();
    let size = encoded_cid_size(&root);
    if size > cid_len {
        return Ok(size);
    }

    let mut ctx = ctx.with_memory()?;
    let mut out_slice = ctx.try_slice_mut(cid_off, cid_len)?;

    root.write_bytes(&mut out_slice)
        .map_err(|err| Trap::new(err.to_string()))?;

    Ok(size)
}

pub fn set_root(caller: Caller<'_, impl Kernel>, cid: u32) -> Result<(), Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let cid = ctx.read_cid(cid)?;
    ctx.data_mut()
        .set_root(cid)
        .map_err(|e| Trap::new(e.to_string()))?; // TODO SYS_ERR this needs to be a system error
    Ok(())
}

pub fn open(caller: Caller<'_, impl Kernel>, cid: u32) -> Result<u32, Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let cid = ctx.read_cid(cid)?;
    Ok(ctx.data_mut().block_open(&cid)?)
}

pub fn create(
    caller: Caller<'_, impl Kernel>,
    codec: u64,
    data_off: u32,
    data_len: u32,
) -> Result<u32, Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let (data, rt) = ctx.try_slice_and_runtime(data_off, data_len)?;
    Ok(rt.block_create(codec, data)?)
}

pub fn cid(
    caller: Caller<'_, impl Kernel>,
    id: u32,
    hash_fun: u64,
    hash_len: u32,
    cid_off: u32,
    cid_len: u32,
) -> Result<u32, Trap> {
    let mut ctx = Context::new(caller);
    let cid = ctx.data_mut().block_link(id, hash_fun, hash_len)?;

    let size = encoded_cid_size(&cid);
    if size > cid_len {
        return Ok(size);
    }

    let mut ctx = ctx.with_memory()?;
    let mut out_slice = ctx.try_slice_mut(cid_off, cid_len)?;

    cid.write_bytes(&mut out_slice)
        .map_err(|err| Trap::new(err.to_string()))?;
    Ok(size)
}

pub fn read(
    caller: Caller<'_, impl Kernel>,
    id: u32,
    offset: u32,
    obuf_off: u32,
    obuf_len: u32,
) -> Result<u32, Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let (data, rt) = ctx.try_slice_and_runtime(obuf_off, obuf_len)?;
    Ok(rt.block_read(id, offset, data)?)
}

pub fn stat(caller: Caller<'_, impl Kernel>, id: u32) -> Result<(u64, u32), Trap> {
    let ctx = Context::new(caller);
    Ok(ctx
        .data()
        .block_stat(id)
        .map(|stat| (stat.codec, stat.size))?)
}
