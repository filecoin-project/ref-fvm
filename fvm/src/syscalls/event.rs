// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use super::Context;
use crate::kernel::Result;
use crate::Kernel;

/// Emits an actor event. The event is split into three raw byte buffers that have
/// been written to Wasm memory. This is done so that the FVM can accurately charge
/// for gas without needing to parse anything inside the FVM.
///
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
    context: Context<'_, impl Kernel>,
    event_off: u32,
    event_len: u32,
    key_off: u32,
    key_len: u32,
    val_off: u32,
    val_len: u32,
) -> Result<()> {
    let raw_event = context.memory.try_slice(event_off, event_len)?;
    let raw_key = context.memory.try_slice(key_off, key_len)?;
    let raw_val = context.memory.try_slice(val_off, val_len)?;
    context.kernel.emit_event(raw_event, raw_key, raw_val)
}
