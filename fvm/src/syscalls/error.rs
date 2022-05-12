//! This module contains code used to convert errors to and from wasmtime traps.
use std::sync::Mutex;

use anyhow::anyhow;
use derive_more::Display;
use fvm_shared::error::ExitCode;
use wasmtime::Trap;

use crate::kernel::ExecutionError;

/// Represents an actor "abort".
#[derive(Debug)]
pub enum Abort {
    /// The actor explicitly aborted with the given exit code (or panicked).
    Exit(ExitCode, String),
    /// The actor ran out of gas.
    OutOfGas,
    /// The system failed with a fatal error.
    Fatal(anyhow::Error),
}

impl Abort {
    /// Convert an execution error into an "abort". We can't directly convert because we need an
    /// exit code, not a syscall error number.
    pub fn from_error(code: ExitCode, e: ExecutionError) -> Self {
        match e {
            ExecutionError::Syscall(e) => Abort::Exit(
                code,
                format!(
                    "actor aborted with an invalid message: {} (code={:?})",
                    e.0, e.1
                ),
            ),
            ExecutionError::OutOfGas => Abort::OutOfGas,
            ExecutionError::Fatal(err) => Abort::Fatal(err),
        }
    }

    /// Just like from_error, but escalating syscall errors as fatal.
    pub fn from_error_as_fatal(e: ExecutionError) -> Self {
        match e {
            ExecutionError::OutOfGas => Abort::OutOfGas,
            ExecutionError::Fatal(e) => Abort::Fatal(e),
            ExecutionError::Syscall(e) => Abort::Fatal(anyhow!("unexpected syscall error: {}", e)),
        }
    }
}

/// Wraps an execution error in a Trap.
impl From<Abort> for Trap {
    fn from(a: Abort) -> Self {
        Trap::from(Box::new(Envelope::wrap(a)) as Box<dyn std::error::Error + Send + Sync + 'static>)
    }
}

/// Unwraps a trap error from an actor into an "abort".
impl From<Trap> for Abort {
    fn from(t: Trap) -> Self {
        use std::error::Error;

        // Actor panic/wasm error.
        if let Some(code) = t.trap_code() {
            return Abort::Exit(ExitCode::SYS_ILLEGAL_INSTRUCTION, code.to_string());
        }

        // Try to get a smuggled error back.
        t.source()
            .and_then(|e| e.downcast_ref::<Envelope>())
            .and_then(|e| e.take())
            // Otherwise, treat this as a fatal error.
            .unwrap_or_else(|| Abort::Fatal(t.into()))
    }
}

/// A super special secret error type for stapling an error to a trap in a way that allows us to
/// pull it back out.
///
/// BE VERY CAREFUL WITH THIS ERROR TYPE: Its source is self-referential.
#[derive(Display, Debug)]
#[display(fmt = "wrapping error")]
struct Envelope {
    inner: Mutex<Option<Abort>>,
}

impl Envelope {
    fn wrap(a: Abort) -> Self {
        Self {
            inner: Mutex::new(Some(a)),
        }
    }
    fn take(&self) -> Option<Abort> {
        self.inner.lock().ok().and_then(|mut a| a.take()).take()
    }
}

impl std::error::Error for Envelope {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self)
    }
}
