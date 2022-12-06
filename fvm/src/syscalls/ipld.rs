// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_shared::sys;

use super::Context;
use crate::kernel::Result;
use crate::Kernel;

// Injected during build
#[no_mangle]
extern "Rust" {
    fn set_syscall_probe(probe: &'static str) -> ();
}

pub fn block_open(context: Context<'_, impl Kernel>, cid: u32) -> Result<sys::out::ipld::IpldOpen> {
    #[cfg(feature = "instrument-syscalls")]
    unsafe { set_syscall_probe("syscall.ipld.block_open") };
    let cid = context.memory.read_cid(cid)?;
    let (id, stat) = context.kernel.block_open(&cid)?;
    Ok(sys::out::ipld::IpldOpen {
        id,
        codec: stat.codec,
        size: stat.size,
    })
}

pub fn block_create(
    context: Context<'_, impl Kernel>,
    codec: u64,
    data_off: u32,
    data_len: u32,
) -> Result<u32> {
    #[cfg(feature = "instrument-syscalls")]
    unsafe { set_syscall_probe("syscall.ipld.block_create") };
    let data = context.memory.try_slice(data_off, data_len)?;
    context.kernel.block_create(codec, data)
}

pub fn block_link(
    context: Context<'_, impl Kernel>,
    id: u32,
    hash_fun: u64,
    hash_len: u32,
    cid_off: u32,
    cid_len: u32,
) -> Result<u32> {
    #[cfg(feature = "instrument-syscalls")]
    unsafe { set_syscall_probe("syscall.ipld.block_link") };
    // Check arguments first.
    context.memory.check_bounds(cid_off, cid_len)?;

    // Link
    let cid = context.kernel.block_link(id, hash_fun, hash_len)?;

    // Return
    context.memory.write_cid(&cid, cid_off, cid_len)
}

pub fn block_read(
    context: Context<'_, impl Kernel>,
    id: u32,
    offset: u32,
    obuf_off: u32,
    obuf_len: u32,
) -> Result<i32> {
    #[cfg(feature = "instrument-syscalls")]
    unsafe { set_syscall_probe("syscall.ipld.block_read") };
    let data = context.memory.try_slice_mut(obuf_off, obuf_len)?;
    context.kernel.block_read(id, offset, data)
}

pub fn block_stat(context: Context<'_, impl Kernel>, id: u32) -> Result<sys::out::ipld::IpldStat> {
    #[cfg(feature = "instrument-syscalls")]
    unsafe { set_syscall_probe("syscall.ipld.block_stat") };
    context
        .kernel
        .block_stat(id)
        .map(|stat| sys::out::ipld::IpldStat {
            codec: stat.codec,
            size: stat.size,
        })
}
