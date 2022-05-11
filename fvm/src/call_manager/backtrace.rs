use std::fmt::Display;

use fvm_ipld_encoding::RawBytes;
use fvm_shared::address::Address;
use fvm_shared::error::{ErrorNumber, ExitCode};
use fvm_shared::{ActorID, MethodNum};

use crate::kernel::SyscallError;

/// A call backtrace records the actors an error was propagated through, from
/// the moment it was emitted. The original error is the _cause_. Backtraces are
/// useful for identifying the root cause of an error.
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

    /// Clear the backtrace.
    pub fn clear(&mut self) {
        self.cause = None;
        self.frames.clear();
    }

    /// Begins a new backtrace. If there is an existing backtrace, this will clear it.
    ///
    /// Note: Backtraces are populated _backwards_. That is, a frame is inserted
    /// every time an actor returns. That's why `begin()` resets any currently
    /// accumulated state, as once an error occurs, we want to track its
    /// propagation all the way up.
    pub fn begin(&mut self, cause: Cause) {
        self.cause = Some(cause);
        self.frames.clear();
    }

    /// Sets the cause of a backtrace.
    ///
    /// This is useful to stamp a backtrace with its cause after the frames
    /// have been collected, such as when we ultimately handle a fatal error at
    /// the top of its propagation chain.
    pub fn set_cause(&mut self, cause: Cause) {
        self.cause = Some(cause);
    }

    /// Push a "frame" (actor exit) onto the backtrace.
    ///
    /// This should be called every time an actor exits.
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

#[derive(Clone, Debug)]
pub struct SyscallCause {
    /// The syscall "module".
    pub module: &'static str,
    /// The syscall function name.
    pub function: &'static str,
    /// The exact syscall error.
    pub error: ErrorNumber,
    /// The informational syscall message.
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct FatalCause {
    /// The alternate-formatted message from the anyhow error.
    pub error_msg: String,
    /// The backtrace, captured if the relevant
    /// [environment variables](https://doc.rust-lang.org/std/backtrace/index.html#environment-variables) are enabled.
    pub backtrace: String,
}

/// The ultimate "cause" of a failed message.
#[derive(Clone, Debug)]
pub enum Cause {
    /// The original cause was a syscall error.
    Syscall(SyscallCause),
    /// The original cause was a fatal error.
    Fatal(FatalCause),
}

impl Cause {
    /// Records a failing syscall as the cause of a backtrace.
    pub fn from_syscall(module: &'static str, function: &'static str, err: SyscallError) -> Self {
        Self::Syscall(SyscallCause {
            module,
            function,
            error: err.1,
            message: err.0,
        })
    }

    /// Records a fatal error as the cause of a backtrace.
    pub fn from_fatal(err: anyhow::Error) -> Self {
        Self::Fatal(FatalCause {
            error_msg: format!("{:#}", err),
            backtrace: err.backtrace().to_string(),
        })
    }
}

impl Display for Cause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cause::Syscall(cause) => {
                write!(
                    f,
                    "{}::{} -- {} ({}: {})",
                    cause.module, cause.function, &cause.message, cause.error as u32, cause.error,
                )
            }
            Cause::Fatal(msg) => {
                write!(
                    f,
                    "[FATAL] Error: {}, Backtrace:\n{}",
                    msg.error_msg, msg.backtrace
                )
            }
        }
    }
}
