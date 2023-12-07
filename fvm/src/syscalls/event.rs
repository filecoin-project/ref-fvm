// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::Context as _;

use super::Context;
use crate::kernel::{ClassifyResult, EventOps, Result};

/// Emits an actor event. The event is split into three raw byte buffers that have
/// been written to Wasm memory. This is done so that the FVM can accurately charge
/// for gas without needing to parse anything inside the FVM.
/// The buffers are serialized as follows:
///  - event_off/event_len: The offset and length tuple of all the event entries
///       flags:u64,key_len:u32,codec:u64,value_len:u32)
///  - key_off/key_len: The offset and length tuple of all entry keys in the event
///  - val_off/val_len: The offset and length tuple of all entry values in the event
///
/// During deserialization, the key_len/value_len stored in each entry will be used
/// to parse the keys and values from the key/value buffers.
///
/// The FVM validates the structural, syntatic, and semantic correctness of the
/// supplied event, and errors with `IllegalArgument` if the payload was invalid.
///
/// Calling this syscall may immediately halt execution with an out of gas error,
/// if such condition arises.
pub fn emit_event(
    context: Context<'_, impl EventOps>,
    event_off: u32,
    event_len: u32,
    key_off: u32,
    key_len: u32,
    val_off: u32,
    val_len: u32,
) -> Result<()> {
    let event_headers = unsafe {
        const EVENT_SIZE: u32 = std::mem::size_of::<fvm_shared::sys::EventEntry>() as u32;
        // assert the alignment so we can safely cast from a byte-slice
        static_assertions::assert_eq_align!(fvm_shared::sys::EventEntry, u8);
        let size = event_len
            .checked_mul(EVENT_SIZE)
            .context("events index out of bounds")
            .or_illegal_argument()?;
        let buf = context.memory.try_slice(event_off, size)?;
        std::slice::from_raw_parts(
            buf.as_ptr() as *const fvm_shared::sys::EventEntry,
            event_len as usize,
        )
    };
    let raw_key = context.memory.try_slice(key_off, key_len)?;
    let raw_val = context.memory.try_slice(val_off, val_len)?;
    context.kernel.emit_event(event_headers, raw_key, raw_val)
}
