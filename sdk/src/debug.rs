// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use lazy_static::lazy_static;
use log::LevelFilter;

use crate::sys;

lazy_static! {
    /// Lazily memoizes if debug mode is enabled.
    static ref DEBUG_ENABLED: bool = unsafe { sys::debug::enabled().unwrap() >= 0 };
}

/// Logs a message on the node.
#[inline]
pub fn log(msg: impl AsRef<str>) {
    let msg = msg.as_ref();
    unsafe {
        sys::debug::log(msg.as_ptr(), msg.len() as u32).unwrap();
    }
}
/// Initialize logging if debugging is enabled.
#[inline(always)]
pub fn init_logging() {
    if enabled() {
        log::set_logger(&Logger).expect("failed to enable logging");
        log::set_max_level(LevelFilter::Trace);
    }
}

/// Begins a new tracing span. Tracing spans are used for debugging. The label and tag are user
/// supplied and can be used by humans to identify the span. The parent is the id of the parent span.
/// The value 0 is reserved for the global span, and should be passed if the span has no explicit parent.
/// Returns the id of the newly created span.
pub fn span_begin(label: impl AsRef<str>, tag: impl AsRef<str>, parent: u64) -> u64 {
    let label = label.as_ref();
    let tag = tag.as_ref();
    unsafe {
        sys::debug::span_begin(
            label.as_ptr(),
            label.len() as u32,
            tag.as_ptr(),
            tag.len() as u32,
            parent,
        )
        .unwrap()
    }
}

/// Ends a tracing span previously created with [`span_begin`].
pub fn span_end(id: u64) {
    unsafe { sys::debug::span_end(id).unwrap() }
}

/// Saves an artifact to the host env. New artifacts with the same name will overwrite old ones
pub fn store_artifact(name: impl AsRef<str>, data: impl AsRef<[u8]>) {
    let name = name.as_ref();
    let data = data.as_ref();
    unsafe {
        sys::debug::store_artifact(
            name.as_ptr(),
            name.len() as u32,
            data.as_ptr(),
            data.len() as u32,
        )
        .unwrap();
    }
}

/// Returns whether debug mode is enabled.
#[inline(always)]
pub fn enabled() -> bool {
    *DEBUG_ENABLED
}

/// Logger is a debug-only logger that uses the FVM syscalls.
struct Logger;

impl log::Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        // TODO: per-level?
        enabled()
    }

    fn log(&self, record: &log::Record) {
        if enabled() {
            log(format!("[{}] {}", record.level(), record.args()));
        }
    }

    fn flush(&self) {}
}
