// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use std::fmt::{Display, Formatter, Result};

use fvm_shared::address::Address;
use fvm_shared::error::{ErrorNumber, ExitCode};
use fvm_shared::{ActorID, MethodNum};

use crate::kernel::SyscallError;

// Assuming 'anyhow' is available in the crate scope for Cause::from_fatal
// use anyhow;

/// A call backtrace records the actors an error was propagated through, from
/// the moment it was emitted. The original error is the _cause_. Backtraces are
/// useful for identifying the root cause of an error in the actor model.
#[derive(Debug, Default, Clone)]
pub struct Backtrace {
    /// The actors through which this error was propagated from bottom (source) to top.
    pub frames: Vec<Frame>,
    /// The last syscall error or fatal error before the first actor in `frames` aborted.
    pub cause: Option<Cause>,
}

impl Display for Backtrace {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        // Frames are displayed in reverse order (top to bottom of the propagation chain)
        // to resemble a traditional call stack trace.
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
    /// Returns true if the backtrace is completely empty (no frames and no cause).
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty() && self.cause.is_none()
    }

    /// Clear the backtrace.
    pub fn clear(&mut self) {
        self.cause = None;
        self.frames.clear();
    }

    /// Begins a new backtrace by setting the initial cause and clearing existing frames.
    ///
    /// Backtraces are populated _backwards_: a frame is inserted every time an actor returns
    /// with an error, tracking its propagation all the way up.
    pub fn begin(&mut self, cause: Cause) {
        self.cause = Some(cause);
        self.frames.clear();
    }

    /// Sets the cause of a backtrace.
    ///
    /// This is useful to stamp a backtrace with its cause after the frames
    /// have been collected, such as when ultimately handling a fatal error at
    /// the top of its propagation chain.
    pub fn set_cause(&mut self, cause: Cause) {
        self.cause = Some(cause);
    }

    /// Push a "frame" (actor exit) onto the backtrace.
    ///
    /// This should be called every time an actor exits with an error code.
    pub fn push_frame(&mut self, frame: Frame) {
        self.frames.push(frame)
    }
}

/// A "frame" in a call backtrace, representing an actor's exit point.
#[derive(Clone, Debug)]
pub struct Frame {
    /// The actor that exited with this code.
    pub source: ActorID,
    /// The method that was invoked on the actor.
    pub method: MethodNum,
    /// The exit code returned by the actor.
    pub code: ExitCode,
    /// The abort message associated with the exit.
    pub message: String,
}

impl Display for Frame {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(
            f,
            "{} (method {}) -- {} ({})",
            Address::new_id(self.source), // Display actor ID as an ID address
            self.method,
            &self.message,
            self.code,
        )
    }
}

/// The ultimate "cause" of a failed message.
#[derive(Clone, Debug)]
pub enum Cause {
    /// The original cause was a syscall error originating in the kernel.
    Syscall {
        /// The syscall "module" (e.g., "crypto").
        module: &'static str,
        /// The syscall function name (e.g., "hash_blake2b").
        function: &'static str,
        /// The exact syscall error number.
        error: ErrorNumber,
        /// The informational syscall message.
        message: String,
    },
    /// The original cause was a fatal error (e.g., a host runtime panic).
    Fatal {
        /// The alternate-formatted message from the anyhow error.
        error_msg: String,
        /// The backtrace, captured if the relevant environment variables are enabled.
        backtrace: String,
    },
}

impl Cause {
    /// Records a failing syscall as the cause of a backtrace.
    pub fn from_syscall(module: &'static str, function: &'static str, err: SyscallError) -> Self {
        Self::Syscall {
            module,
            function,
            error: err.1,
            message: err.0,
        }
    }

    /// Records a fatal error as the cause of a backtrace.
    /// NOTE: This function requires the 'anyhow' crate to be accessible in the environment.
    pub fn from_fatal(err: anyhow::Error) -> Self {
        Self::Fatal {
            error_msg: format!("{:#}", err),
            backtrace: err.backtrace().to_string(),
        }
    }
}

impl Display for Cause {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Cause::Syscall {
                module,
                function,
                error,
                message,
            } => {
                // Simplified display format: only show the error number once.
                write!(
                    f,
                    "{module}::{function} -- {message} (code: {code})",
                    module = module,
                    function = function,
                    message = message,
                    // Format ErrorNumber as its raw u32 value for clarity.
                    code = *error as u32,
                )
            }
            Cause::Fatal {
                error_msg,
                backtrace,
            } => {
                // Prints the fatal error message followed by the detailed Rust backtrace.
                write!(f, "[FATAL] Error: {}\n{}", error_msg, backtrace)
            }
        }
    }
}
