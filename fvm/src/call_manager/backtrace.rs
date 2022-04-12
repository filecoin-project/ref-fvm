use std::fmt::Display;

use fvm_ipld_encoding::RawBytes;
use fvm_shared::address::Address;
use fvm_shared::error::{ErrorNumber, ExitCode};
use fvm_shared::{ActorID, MethodNum};

use crate::kernel::SyscallError;

/// A call backtrace records _why_ an actor exited with a specific error code.
#[derive(Debug, Default, Clone)]
pub struct Backtrace {
    /// The actors through which this error was propagated from bottom (source) to top.
    pub frames: Vec<Frame>,
    /// The last syscall error before the first actor in `frames` aborted.
    pub cause: Option<Cause>,
}

impl Display for Backtrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, frame) in self.frames.iter().rev().enumerate() {
            writeln!(f, "{:02}: {}", i, frame)?;
        }
        if let Some(cause) = &self.cause {
            writeln!(f, "--> caused by: {}", cause)?;
        }
        Ok(())
    }
}

impl Backtrace {
    /// Returns true if the backtrace is completely empty.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty() && self.cause.is_none()
    }

    /// Clear the backtrace. This should be called:
    ///
    /// 1. Before all syscalls except "abort"
    /// 2. After an actor returns with a 0 exit code.
    pub fn clear(&mut self) {
        self.cause = None;
        self.frames.clear();
    }

    /// Set the backtrace cause. If there is an existing backtrace, this will clear it.
    pub fn set_cause(&mut self, cause: Cause) {
        self.cause = Some(cause);
        self.frames.clear();
    }

    /// Push a "frame" (actor exit) onto the backtrace.
    pub fn push_frame(&mut self, frame: Frame) {
        self.frames.push(frame)
    }
}

/// A "frame" in a call backtrace.
#[derive(Clone, Debug)]
pub struct Frame {
    /// The actor that exited with this code.
    pub source: ActorID,
    /// The method that was invoked.
    pub method: MethodNum,
    /// The parameters passed to this method.
    pub params: RawBytes,
    /// The exit code.
    pub code: ExitCode,
    /// The abort message.
    pub message: String,
}

impl Display for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (method {}) -- {} ({})",
            Address::new_id(self.source),
            self.method,
            &self.message,
            self.code,
        )
    }
}

/// The ultimate "cause" of a failed message.
#[derive(Clone, Debug)]
pub struct Cause {
    /// The syscall "module".
    pub module: &'static str,
    /// The syscall function name.
    pub function: &'static str,
    /// The exact syscall error.
    pub error: ErrorNumber,
    /// The informational syscall message.
    pub message: String,
}

impl Cause {
    pub fn new(module: &'static str, function: &'static str, err: SyscallError) -> Self {
        Self {
            module,
            function,
            error: err.1,
            message: err.0,
        }
    }
}

impl Display for Cause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}::{} -- {} ({}: {})",
            self.module, self.function, &self.message, self.error as u32, self.error,
        )
    }
}
