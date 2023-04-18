//! This module contains code used to convert errors to and from wasmtime traps.
use anyhow::anyhow;
use fvm_shared::error::ExitCode;
use wasmtime::Trap;

use crate::kernel::ExecutionError;

/// Represents an actor "abort".
#[derive(Debug, thiserror::Error)]
pub enum Abort {
    /// The actor explicitly aborted with the given exit code (or panicked).
    #[error("exit with code {0} ({1})")]
    Exit(ExitCode, String),
    /// The actor ran out of gas.
    #[error("out of gas")]
    OutOfGas,
    /// The system failed with a fatal error.
    #[error("fatal error: {0}")]
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

/// Unwraps a trap error from an actor into an "abort".
impl From<anyhow::Error> for Abort {
    fn from(e: anyhow::Error) -> Self {
        if let Some(trap) = e.downcast_ref::<Trap>() {
            return match trap {
                | Trap::MemoryOutOfBounds
                | Trap::TableOutOfBounds
                | Trap::IndirectCallToNull
                | Trap::BadSignature
                | Trap::IntegerOverflow
                | Trap::IntegerDivisionByZero
                | Trap::BadConversionToInteger
                | Trap::UnreachableCodeReached

                // Should require the atomic feature to be enabled, but we might as well just
                // handle this.
                | Trap::HeapMisaligned
                | Trap::AtomicWaitNonSharedMemory

                // I think this is fatal? But I'm not sure.
                | Trap::StackOverflow => Abort::Exit(
                    ExitCode::SYS_ILLEGAL_INSTRUCTION,
                    trap.to_string(),
                ),
                _ => Abort::Fatal(anyhow!("unexpected wasmtime trap: {}", trap)),
            };
        };
        match e.downcast::<Abort>() {
            Ok(abort) => abort,
            Err(e) => Abort::Fatal(e),
        }
    }
}
