use fvm_shared::event::ActorEvent;

use crate::{sys, SyscallResult};

pub fn emit_event(evt: &ActorEvent) -> SyscallResult<()> {
    let encoded = fvm_ipld_encoding::to_vec(evt).expect("failed to marshal actor event");
    let entries = encoded.as_slice();

    unsafe { sys::event::emit_event(entries.as_ptr(), entries.len() as u32) }
}
