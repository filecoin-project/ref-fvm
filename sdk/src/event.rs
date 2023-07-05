// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use crate::{sys, SyscallResult};
use byteorder::{BigEndian, ByteOrder};
use fvm_shared::event::ActorEvent;

pub fn emit_event(evt: &ActorEvent) -> SyscallResult<()> {
    // we manually serialize the ActorEvent (not using CBOR) into three byte arrays so
    // we can accurately charge gas without needing to parse anything inside the FVM
    const BYTES_PER_ENTRY: usize = 24;

    let mut total_key_len: usize = 0;
    let mut total_val_len: usize = 0;

    let mut v = vec![0u8; evt.entries.len() * BYTES_PER_ENTRY];
    for i in 0..evt.entries.len() {
        let e = &evt.entries[i];
        let offset = i * BYTES_PER_ENTRY;

        let view = &mut v[offset..offset + BYTES_PER_ENTRY];
        BigEndian::write_u64(&mut view[..8], e.flags.bits());
        BigEndian::write_u32(&mut view[8..12], e.key.len() as u32);
        BigEndian::write_u64(&mut view[12..20], e.codec);
        BigEndian::write_u32(&mut view[20..24], e.value.len() as u32);

        total_key_len += e.key.len();
        total_val_len += e.value.len();
    }

    let mut keys = vec![0u8; total_key_len];
    let mut offset: usize = 0;
    for i in 0..evt.entries.len() {
        let e = &evt.entries[i];
        keys[offset..offset + e.key.len()].copy_from_slice(e.key.as_bytes());
        offset += e.key.len();
    }

    let mut values = vec![0u8; total_val_len];
    let mut offset: usize = 0;
    for i in 0..evt.entries.len() {
        let e = &evt.entries[i];
        values[offset..offset + e.value.len()].copy_from_slice(e.value.as_slice());
        offset += e.value.len();
    }

    unsafe {
        sys::event::emit_event(
            v.as_slice().as_ptr(),
            v.as_slice().len() as u32,
            keys.as_slice().as_ptr(),
            keys.as_slice().len() as u32,
            values.as_slice().as_ptr(),
            values.as_slice().len() as u32,
        )
    }
}
