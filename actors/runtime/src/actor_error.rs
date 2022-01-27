use fvm_shared::error::ExitCode;
use thiserror::Error;

/// TODO fix error system; actor errors should be transparent to the VM.
/// The error type that gets returned by actor method calls.
#[derive(Error, Debug, Clone, PartialEq)]
#[error("ActorError(exit_code: {exit_code:?}, msg: {msg})")]
pub struct ActorError {
    /// The exit code for this invocation, must not be `0`.
    exit_code: ExitCode,
    /// Message for debugging purposes,
    msg: String,
}

impl ActorError {
    pub fn new(exit_code: ExitCode, msg: String) -> Self {
        Self { exit_code, msg }
    }

    /// Returns the exit code of the error.
    pub fn exit_code(&self) -> ExitCode {
        self.exit_code
    }

    /// Returns true when the exit code is `Ok`.
    pub fn is_ok(&self) -> bool {
        self.exit_code == ExitCode::Ok
    }

    /// Error message of the actor error.
    pub fn msg(&self) -> &str {
        &self.msg
    }

    /// Prefix error message with a string message.
    pub fn wrap(mut self, msg: impl AsRef<str>) -> Self {
        self.msg = format!("{}: {}", msg.as_ref(), self.msg);
        self
    }
}

// TODO former EncodingError
impl From<fvm_shared::encoding::Error> for ActorError {
    fn from(e: fvm_shared::encoding::Error) -> Self {
        Self {
            exit_code: ExitCode::ErrSerialization,
            msg: e.to_string(),
        }
    }
}

// TODO former CborError
impl From<fvm_shared::encoding::error::Error> for ActorError {
    fn from(e: fvm_shared::encoding::error::Error) -> Self {
        Self {
            exit_code: ExitCode::ErrSerialization,
            msg: e.to_string(),
        }
    }
}

/// Converts an actor deletion error into an actor error with the appropriate exit code. This
/// facilitates propagation.
#[cfg(feature = "runtime-wasm")]
impl From<fvm_sdk::error::ActorDeleteError> for ActorError {
    fn from(e: fvm_sdk::error::ActorDeleteError) -> Self {
        use fvm_sdk::error::ActorDeleteError::*;
        Self {
            // FIXME: These shouldn't be "system" errors, but we're trying to match existing
            // behavior here.
            exit_code: match e {
                BeneficiaryIsSelf => ExitCode::SysErrIllegalActor,
                BeneficiaryDoesNotExist => ExitCode::SysErrIllegalArgument,
            },
            msg: e.to_string(),
        }
    }
}

/// Converts a no-state error into an an actor error with the appropriate exit code (illegal actor).
/// This facilitates propagation.
#[cfg(feature = "runtime-wasm")]
impl From<fvm_sdk::error::NoStateError> for ActorError {
    fn from(e: fvm_sdk::error::NoStateError) -> Self {
        Self {
            // FIXME: These shouldn't be "system" errors, but we're trying to match existing
            // behavior here.
            exit_code: ExitCode::SysErrIllegalActor,
            msg: e.to_string(),
        }
    }
}

/// Performs conversions from SyscallResult, whose error type is ExitCode,
/// to ActorErrors. This facilitates propagation.
impl From<ExitCode> for ActorError {
    fn from(e: ExitCode) -> Self {
        ActorError {
            exit_code: e,
            msg: "".to_string(),
        }
    }
}

/// Convenience macro for generating Actor Errors
#[macro_export]
macro_rules! actor_error {
    // Error with only one stringable expression
    ( $code:ident; $msg:expr ) => { $crate::ActorError::new(fvm_shared::error::ExitCode::$code, $msg.to_string()) };

    // String with positional arguments
    ( $code:ident; $msg:literal $(, $ex:expr)+ ) => {
        $crate::ActorError::new(fvm_shared::error::ExitCode::$code, format!($msg, $($ex,)*))
    };

    // Error with only one stringable expression, with comma separator
    ( $code:ident, $msg:expr ) => { $crate::actor_error!($code; $msg) };

    // String with positional arguments, with comma separator
    ( $code:ident, $msg:literal $(, $ex:expr)+ ) => {
        $crate::actor_error!($code; $msg $(, $ex)*)
    };
}
