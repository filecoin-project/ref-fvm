use std::fmt::Formatter;

use fvm_ipld_encoding::repr::*;
use num_derive::FromPrimitive;
use serde::{Deserializer, Serializer};
use thiserror::Error;

/// ExitCode defines the exit code from the VM invocation.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct ExitCode {
    value: u32,
}

impl ExitCode {
    /// The code indicating successful execution.
    pub const OK: ExitCode = ExitCode::new(0);

    /// The lowest exit code that an actor may abort with.
    pub const FIRST_UNRESERVED_EXIT_CODE: u32 = 16;

    pub const fn new(value: u32) -> Self {
        Self { value }
    }

    pub fn value(self) -> u32 {
        self.value
    }

    /// Returns true if the exit code indicates success.
    pub fn is_success(self) -> bool {
        self.value == 0
    }

    /// Returns true if the error code is in the range of exit codes reserved for the VM
    /// (including Ok).
    pub fn is_system_error(self) -> bool {
        self.value < (Self::FIRST_UNRESERVED_EXIT_CODE)
    }
}

impl std::fmt::Display for ExitCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl serde::Serialize for ExitCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.value)
    }
}

impl<'de> serde::Deserialize<'de> for ExitCode {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(ExitCode { value: 0 }) // FIXME figure this out
    }
}

/// Enumerates exit codes which originate inside the VM.
/// These values may not be used by actors when aborting.
pub struct SystemExitCode {}

impl SystemExitCode {
    /// Indicates the message sender doesn't exist.
    pub const SENDER_INVALID: ExitCode = ExitCode::new(1);
    /// Indicates that the message sender was not in a valid state to send this message.
    /// Either:
    /// - The sender's nonce nonce didn't match the message nonce.
    /// - The sender didn't have the funds to cover the message gas.
    pub const SENDER_STATE_INVALID: ExitCode = ExitCode::new(2);
    /// Indicates failure to find a method in an actor.
    pub const INVALID_METHOD: ExitCode = ExitCode::new(3); // FIXME: reserved
    /// Indicates the message receiver trapped (panicked).
    pub const ILLEGAL_INSTRUCTION: ExitCode = ExitCode::new(4);
    /// Indicates the message receiver doesn't exist and can't be automatically created
    pub const INVALID_RECEIVER: ExitCode = ExitCode::new(5);
    /// Indicates the message sender didn't have the requisite funds.
    pub const INSUFFICIENT_FUNDS: ExitCode = ExitCode::new(6);
    /// Indicates message execution (including subcalls) used more gas than the specified limit.
    pub const OUT_OF_GAS: ExitCode = ExitCode::new(7);
    // REVIEW: I restored this in order to map the syscall error number ErrorNumber::IllegalOperation
    // Should it use ILLEGAL_INSTRUCTION instead?
    pub const ILLEGAL_ACTOR: ExitCode = ExitCode::new(8);
    /// Indicates the message receiver aborted with a reserved exit code.
    pub const ILLEGAL_EXIT_CODE: ExitCode = ExitCode::new(9);
    /// Indicates an internal VM assertion failed.
    pub const ASSERTION_FAILED: ExitCode = ExitCode::new(10);
    /// Indicates the actor returned a block handle that doesn't exist
    pub const MISSING_RETURN: ExitCode = ExitCode::new(11);

    pub const RESERVED_12: ExitCode = ExitCode::new(12);
    pub const RESERVED_13: ExitCode = ExitCode::new(13);
    pub const RESERVED_14: ExitCode = ExitCode::new(14);
    pub const RESERVED_15: ExitCode = ExitCode::new(15);
}

/// Enumerates standard exit codes according to the built-in actors' calling convention.
pub struct StandardExitCode {}

impl StandardExitCode {
    /// Indicates a method parameter is invalid.
    pub const ILLEGAL_ARGUMENT: ExitCode = ExitCode::new(16);
    /// Indicates a requested resource does not exist.
    pub const NOT_FOUND: ExitCode = ExitCode::new(17);
    /// Indicates an action is disallowed.
    pub const FORBIDDEN: ExitCode = ExitCode::new(18);
    /// Indicates a balance of funds is insufficient.
    pub const INSUFFICIENT_FUNDS: ExitCode = ExitCode::new(19);
    /// Indicates an actor's internal state is invalid.
    pub const ILLEGAL_STATE: ExitCode = ExitCode::new(20);
    /// Indicates de/serialization failure within actor code.
    pub const SERIALIZATION: ExitCode = ExitCode::new(21);
    /// Indicates the actor cannot handle this message.
    pub const UNHANDLED_MESSAGE: ExitCode = ExitCode::new(22);
    /// Indicates the actor failed with an unspecified error.
    pub const UNSPECIFIED: ExitCode = ExitCode::new(23);

    pub const RESERVED_24: ExitCode = ExitCode::new(24);
    pub const RESERVED_25: ExitCode = ExitCode::new(25);
    pub const RESERVED_26: ExitCode = ExitCode::new(26);
    pub const RESERVED_27: ExitCode = ExitCode::new(27);
    pub const RESERVED_28: ExitCode = ExitCode::new(28);
    pub const RESERVED_29: ExitCode = ExitCode::new(29);
    pub const RESERVED_30: ExitCode = ExitCode::new(30);
    pub const RESERVED_31: ExitCode = ExitCode::new(31);
}

#[repr(u32)]
#[derive(Copy, Clone, Eq, Debug, PartialEq, Error, FromPrimitive)]
pub enum ErrorNumber {
    IllegalArgument = 1,
    IllegalOperation = 2,
    LimitExceeded = 3,
    AssertionFailed = 4,
    InsufficientFunds = 5,
    NotFound = 6,
    InvalidHandle = 7,
    IllegalCid = 8,
    IllegalCodec = 9,
    Serialization = 10,
    Forbidden = 11,
}

impl std::fmt::Display for ErrorNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use ErrorNumber::*;
        f.write_str(match *self {
            IllegalArgument => "illegal argument",
            IllegalOperation => "illegal operation",
            LimitExceeded => "limit exceeded",
            AssertionFailed => "filecoin assertion failed",
            InsufficientFunds => "insufficient funds",
            NotFound => "resource not found",
            InvalidHandle => "invalid ipld block handle",
            IllegalCid => "illegal cid specification",
            IllegalCodec => "illegal ipld codec",
            Serialization => "serialization error",
            Forbidden => "operation forbidden",
        })
    }
}
