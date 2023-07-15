// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! Syscalls for debugging.

super::fvm_syscalls! {
    module = "debug";

    /// Returns if we're in debug mode. A zero or positive return value means
    /// yes, a negative return value means no.
    pub fn enabled() -> Result<i32>;

    /// Logs a message on the node.
    pub fn log(message: *const u8, message_len: u32) -> Result<()>;

    // TODO Docs
    pub fn span_begin(label: *const u8, label_len: u32, tag: *const u8, tag_len: u32, parent: u64) -> Result<u64>;

    // TODO Docs
    pub fn span_end(parent: u64) -> Result<()>;

    /// Save data as a debug artifact on the node.
    pub fn store_artifact(name_off: *const u8, name_len: u32, data_off: *const u8, data_len: u32) -> Result<()>;
}
