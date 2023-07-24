// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use crate::{sys, SyscallResult};
use fvm_shared::event::ActorEvent;

pub fn emit_event(evt: &ActorEvent) -> SyscallResult<()> {
    // we manually serialize the ActorEvent (not using CBOR) into three byte arrays so
    // we can accurately charge gas without needing to parse anything inside the FVM
    let mut total_key_len: usize = 0;
    let mut total_val_len: usize = 0;

    let mut fixed_entries = Vec::with_capacity(evt.entries.len());
    for i in 0..evt.entries.len() {
        let e = &evt.entries[i];

        fixed_entries.push(fvm_shared::sys::EventEntry {
            flags: e.flags,
            codec: e.codec,
            key_len: e.key.len() as u32,
            val_len: e.value.len() as u32,
        });

        total_key_len += e.key.len();
        total_val_len += e.value.len();
    }

    let mut keys = Vec::with_capacity(total_key_len);
    for i in 0..evt.entries.len() {
        keys.extend_from_slice(evt.entries[i].key.as_bytes());
    }

    let mut values = Vec::with_capacity(total_val_len);
    for i in 0..evt.entries.len() {
        values.extend_from_slice(evt.entries[i].value.as_slice());
    }

    unsafe {
        sys::event::emit_event(
            fixed_entries.as_ptr(),
            fixed_entries.len() as u32,
            keys.as_ptr(),
            keys.len() as u32,
            values.as_ptr(),
            values.len() as u32,
        )
    }
}
