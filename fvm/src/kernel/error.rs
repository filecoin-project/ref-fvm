use std::fmt::Display;

use derive_more::Display;
use fvm_shared::error::ExitCode;

/// Execution result.
pub type Result<T> = std::result::Result<T, ExecutionError>;

/// Convenience macro for generating Actor Errors
#[macro_export]
macro_rules! syscall_error {
    // Error with only one stringable expression
    ( $code:ident; $msg:expr ) => { $crate::kernel::SyscallError::new(fvm_shared::error::ExitCode::$code, $msg) };

    // String with positional arguments
    ( $code:ident; $msg:literal $(, $ex:expr)+ ) => {
        $crate::kernel::SyscallError::new(fvm_shared::error::ExitCode::$code, format_args!($msg, $($ex,)*))
    };

    // Error with only one stringable expression, with comma separator
    ( $code:ident, $msg:expr ) => { $crate::syscall_error!($code; $msg) };

    // String with positional arguments, with comma separator
    ( $code:ident, $msg:literal $(, $ex:expr)+ ) => {
        $crate::syscall_error!($code; $msg $(, $ex)*)
    };
}

// NOTE: this intentionally does not implemnent error so we can make the context impl work out
// below.
#[derive(Display, Debug)]
pub enum ExecutionError {
    Syscall(SyscallError),
    Fatal(anyhow::Error),
}

// NOTE: this is the _only_ from impl we provide. Otherwise, we expect the user to explicitly
// select between the two options.
impl From<SyscallError> for ExecutionError {
    fn from(e: SyscallError) -> Self {
        ExecutionError::Syscall(e)
    }
}

pub trait ClassifyResult: Sized {
    type Value;
    type Error;

    // TODO: may need a custom trait for conversions because into will be a bit restrictive.
    fn or_fatal(self) -> Result<Self::Value>
    where
        Self::Error: Into<anyhow::Error>;
    fn or_error(self, code: ExitCode) -> Result<Self::Value>
    where
        Self::Error: Display;

    fn or_illegal_argument(self) -> Result<Self::Value>
    where
        Self::Error: Display,
    {
        self.or_error(ExitCode::ErrIllegalArgument)
    }
}

impl<T, E> ClassifyResult for std::result::Result<T, E> {
    type Value = T;
    type Error = E;

    fn or_fatal(self) -> Result<Self::Value>
    where
        Self::Error: Into<anyhow::Error>,
    {
        self.map_err(|e| ExecutionError::Fatal(e.into()))
    }
    fn or_error(self, code: ExitCode) -> Result<Self::Value>
    where
        Self::Error: Display,
    {
        self.map_err(|e| ExecutionError::Syscall(SyscallError(e.to_string(), code)))
    }
}

/// The FVM's equivalent of `anyhow::Context`. This is intentionally only implemented on
/// `ExecutionError` and `Result<T, ExecutionError>` so `anyhow::Context` can be imported at the
/// same time.
pub trait Context {
    type WithContext;
    fn context<D>(self, context: D) -> Self::WithContext
    where
        D: Display;
    fn with_context<D, F>(self, cfn: F) -> Self::WithContext
    where
        D: Display,
        F: FnOnce() -> D;
}

impl<T> Context for Result<T> {
    type WithContext = Result<T>;
    fn context<D: Display>(self, context: D) -> Self::WithContext {
        self.map_err(|e| e.context(context))
    }

    fn with_context<D, F>(self, cfn: F) -> Self::WithContext
    where
        D: Display,
        F: FnOnce() -> D,
    {
        self.map_err(|e| e.with_context(cfn))
    }
}

impl Context for ExecutionError {
    type WithContext = Self;
    fn context<D: Display>(self, context: D) -> Self {
        use ExecutionError::*;
        match self {
            Syscall(e) => Syscall(SyscallError(format!("{}: {}", context, e.0), e.1)),
            Fatal(e) => Fatal(e.context(context.to_string())),
        }
    }

    fn with_context<D, F>(self, cfn: F) -> Self::WithContext
    where
        D: Display,
        F: FnOnce() -> D,
    {
        self.context(cfn())
    }
}

// We only use this when converting to a fatal error, so we throw away the error code.
//
// TODO: Ideally we wouldn't implement this conversion as it's a bit dangerous.
impl From<ExecutionError> for anyhow::Error {
    fn from(e: ExecutionError) -> Self {
        use ExecutionError::*;
        match e {
            Syscall(err) => anyhow::anyhow!(err.0),
            Fatal(err) => err,
        }
    }
}

/// Represents an error from a syscall. It can optionally contain a
/// syscall-advised exit code for the kind of error that was raised.
/// We may want to add an optional source error here.
///
/// Automatic conversions from String are provided, with no advised exit code.
#[derive(thiserror::Error, Debug, Clone)]
#[error("syscall error: {0} (exit_code={1:?})")]
pub struct SyscallError(pub String, pub ExitCode);

impl SyscallError {
    pub fn new<D: Display>(c: ExitCode, d: D) -> Self {
        SyscallError(d.to_string(), c)
    }
}

impl ExecutionError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            ExecutionError::Fatal(_) => ExitCode::ErrPlaceholder, // same as fatal before
            ExecutionError::Syscall(SyscallError(_, exit_code)) => *exit_code,
        }
    }
}
