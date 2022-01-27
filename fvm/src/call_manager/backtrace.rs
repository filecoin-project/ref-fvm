use fvm_shared::error::ExitCode;
use fvm_shared::ActorID;

use crate::kernel::SyscallError;

/// A call backtrace records _why_ an actor exited with a specific error code.
#[derive(Debug, Default, Clone)]
pub struct Backtrace {
    /// The actors through which this error was propegated from bottom (source) to top.
    frames: Vec<Frame>,
    /// The last syscall error before the first actor in `frames` aborted.
    cause: Option<SyscallError>,
}

// TODO: Include the call parameters? I can probably even include internal actor backtraces if in
// debug mode.

/// A "frame" in a call backtrace.
#[derive(Clone, Debug)]
pub struct Frame {
    /// The actor that exited with this code.
    pub source: ActorID,
    /// The exit code.
    pub code: ExitCode,
    /// The error message.
    pub message: String,
}

impl Backtrace {
    pub fn clear(&mut self) {
        self.cause = None;
        self.frames.clear();
    }

    pub fn set_cause(&mut self, e: SyscallError) {
        self.cause = Some(e);
        self.frames.clear();
    }
    pub fn push_exit(&mut self, actor: ActorID, code: ExitCode, message: String) {
        self.frames.push(Frame {
            source: actor,
            code,
            message,
        });
    }
}
