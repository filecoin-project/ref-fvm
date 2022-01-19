//! This module contains code used to convert errors to and from wasmtime traps.
use std::sync::Mutex;

use anyhow::Context as _;
use derive_more::Display;
use fvm_shared::error::ExitCode;
use num_traits::FromPrimitive;
use wasmtime::Trap;

use crate::call_manager::InvocationResult;
use crate::kernel::{ClassifyResult, ExecutionError};

/// Wraps an execution error in a Trap.
pub fn trap_from_error(e: ExecutionError) -> Trap {
    Trap::from(
        Box::new(ErrorEnvelope::wrap(e)) as Box<dyn std::error::Error + Send + Sync + 'static>
    )
}

/// Wraps an exit code in a Trap.
pub fn trap_from_code(code: ExitCode) -> Trap {
    Trap::i32_exit(code as i32)
}

/// Unwraps a trap error from an actor into one of:
///
/// 1. An invocation result with an exit code (if the trap is recoverable).
/// 2. An "illegal actor" syscall error if the trap is caused by a WASM error.
/// 3. A syscall error if the trap is neither fatal nor recoverable (currently just "out of gas").
/// 4. A fatal error otherwise.
pub fn unwrap_trap(e: Trap) -> crate::kernel::Result<InvocationResult> {
    use std::error::Error;

    if let Some(status) = e.i32_exit_status() {
        return Ok(InvocationResult::Failure(
            ExitCode::from_i32(status)
                .with_context(|| format!("invalid exit code: {}", status))
                .or_fatal()?,
        ));
    }

    if e.trap_code().is_some() {
        return Ok(InvocationResult::Failure(ExitCode::SysErrActorPanic));
    }

    // Do whatever we can to pull the original error back out (if it exists).
    Err(e
        .source()
        .and_then(|e| e.downcast_ref::<ErrorEnvelope>())
        .and_then(|e| e.inner.lock().ok())
        .and_then(|mut e| e.take())
        .unwrap_or_else(|| ExecutionError::Fatal(e.into())))
}

/// A super special secret error type for stapling an error to a trap in a way that allows us to
/// pull it back out.
///
/// BE VERY CAREFUL WITH THIS ERROR TYPE: Its source is self-referential.
#[derive(Display, Debug)]
#[display(fmt = "wrapping error")]
struct ErrorEnvelope {
    inner: Mutex<Option<ExecutionError>>,
}

impl ErrorEnvelope {
    fn wrap(e: ExecutionError) -> Self {
        Self {
            inner: Mutex::new(Some(e)),
        }
    }
}

impl std::error::Error for ErrorEnvelope {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self)
    }
}
