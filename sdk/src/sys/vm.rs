// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! Syscalls for interacting with the VM.

#[doc(inline)]
pub use fvm_shared::sys::out::vm::MessageContext;

super::fvm_syscalls! {
    module = "vm";

    /// Abort execution with the given code and optional message and data for the return value.
    /// The code and return value are recorded in the receipt, the message is for debugging only.
    ///
    /// # Arguments
    ///
    /// - `code` is the [`ExitCode`][fvm_shared::error::ExitCode] to abort with.
    ///   If this code is zero, then the exit indicates a successful non-local return from
    ///   the current execution context.
    ///   If this code is not zero and less than the [minimum "user" exit
    ///   code][fvm_shared::error::ExitCode::FIRST_USER_EXIT_CODE], it will be replaced with
    ///   [`SYS_ILLEGAL_EXIT_CODE`][fvm_shared::error::ExitCode::SYS_ILLEGAL_EXIT_CODE].
    /// - `blk_id` is the optional data block id; it should be 0 if there are no data attached to
    ///   this exit.
    /// - `message_off` and `message_len` specify the offset and length (in wasm memory) of an
    ///   optional debug message associated with this abort. These parameters may be null/0 and will
    ///   be ignored if invalid.
    ///
    /// # Errors
    ///
    /// None. This function doesn't return.
    pub fn exit(code: u32, blk_id: u32, message_off: *const u8, message_len: u32) -> !;

    /// Returns the details about the message causing this invocation.
    ///
    /// # Errors
    ///
    /// None
    pub fn message_context() -> Result<MessageContext>;
}
