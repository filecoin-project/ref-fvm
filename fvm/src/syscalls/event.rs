// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_shared::event::ActorEvent;

use super::Context;
use crate::kernel::Result;
use crate::Kernel;

// Injected during build
#[no_mangle]
extern "Rust" {
    fn set_syscall_probe(probe: &'static str) -> ();
}

pub fn emit_event(
    context: Context<'_, impl Kernel>,
    event_off: u32, // ActorEvent
    event_len: u32,
) -> Result<()> {
    #[cfg(feature = "instrument-syscalls")]
    unsafe { set_syscall_probe("syscall.event.emit_event") };
    let evt: ActorEvent = context.memory.read_cbor(event_off, event_len)?;
    context.kernel.emit_event(evt)
}
