// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::convert::TryFrom;
use std::error::Error as StdError;

use fvm_shared::encoding::{error::Error as CborError, EncodingError};
use fvm_shared::error::{CallError, ExitCode};
use ipld_amt::AmtError;
use ipld_hamt::HamtError;

/// Trait to allow multiple error types to be able to be converted into a `CallError`.
pub trait CallErrorConversions {
    /// Convert a dynamic std Error into a `CallError`. If the error cannot be converted
    /// into an CallError automatically, use the provided `ExitCode` to generate a new error.
    fn convert_default(self, default_exit_code: ExitCode, msg: impl AsRef<str>) -> CallError;

    /// Convert a dynamic std Error into a `CallError`. If the error cannot be converted
    /// then it will escalate the error to a fatal error.
    fn convert_fatal(self, msg: impl AsRef<str>) -> CallError;

    /// Wrap the error with a message, without overwriting an exit code.
    fn convert_wrap(self, msg: impl AsRef<str>) -> Box<dyn StdError>;
}

impl CallErrorConversions for Box<dyn StdError> {
    fn convert_default(self, default_exit_code: ExitCode, msg: impl AsRef<str>) -> CallError {
        match try_convert(self) {
            Ok(call_error) => call_error.wrap(msg),
            Err(other) => CallError::new(default_exit_code, format!("{}: {}", msg.as_ref(), other)),
        }
    }
    fn convert_fatal(self, msg: impl AsRef<str>) -> CallError {
        match try_convert(self) {
            Ok(call_error) => call_error.wrap(msg),
            Err(other) => CallError::new_fatal(format!("{}: {}", msg.as_ref(), other)),
        }
    }
    fn convert_wrap(self, msg: impl AsRef<str>) -> Box<dyn StdError> {
        match try_convert(self) {
            Ok(call_error) => Box::new(call_error.wrap(msg)),
            Err(other) => format!("{}: {}", msg.as_ref(), other).into(),
        }
    }
}

impl CallErrorConversions for AmtError {
    fn convert_default(self, default_exit_code: ExitCode, msg: impl AsRef<str>) -> CallError {
        match self {
            AmtError::Dynamic(e) => e.convert_default(default_exit_code, msg),
            other => CallError::new(default_exit_code, format!("{}: {}", msg.as_ref(), other)),
        }
    }
    fn convert_fatal(self, msg: impl AsRef<str>) -> CallError {
        match self {
            AmtError::Dynamic(e) => e.convert_fatal(msg),
            other => CallError::new_fatal(format!("{}: {}", msg.as_ref(), other)),
        }
    }
    fn convert_wrap(self, msg: impl AsRef<str>) -> Box<dyn StdError> {
        match self {
            AmtError::Dynamic(e) => e.convert_wrap(msg),
            other => format!("{}: {}", msg.as_ref(), other).into(),
        }
    }
}

impl CallErrorConversions for HamtError {
    fn convert_default(self, default_exit_code: ExitCode, msg: impl AsRef<str>) -> CallError {
        match self {
            HamtError::Dynamic(e) => e.convert_default(default_exit_code, msg),
            other => CallError::new(default_exit_code, format!("{}: {}", msg.as_ref(), other)),
        }
    }
    fn convert_fatal(self, msg: impl AsRef<str>) -> CallError {
        match self {
            HamtError::Dynamic(e) => e.convert_fatal(msg),
            other => CallError::new_fatal(format!("{}: {}", msg.as_ref(), other)),
        }
    }
    fn convert_wrap(self, msg: impl AsRef<str>) -> Box<dyn StdError> {
        match self {
            HamtError::Dynamic(e) => e.convert_wrap(msg),
            other => format!("{}: {}", msg.as_ref(), other).into(),
        }
    }
}

/// Attempts to downcast a `Box<dyn std::error::Error>` into an actor error.
/// Returns `Ok` with the actor error if it can be converted automatically
/// and returns `Err` with the original error if it cannot.
fn try_convert(error: Box<dyn StdError>) -> Result<CallError, Box<dyn StdError>> {
    // Check if error is CallError, return as such
    let error = match error.downcast::<CallError>() {
        Ok(actor_err) => return Ok(*actor_err),
        Err(other) => other,
    };

    // Check if error is Encoding error, if so return `ErrSerialization`
    let error = match error.downcast::<EncodingError>() {
        Ok(enc_error) => {
            return Ok(CallError::new(
                ExitCode::ErrSerialization,
                enc_error.to_string(),
            ))
        }
        Err(other) => other,
    };

    // Check also for Cbor error to be safe. All should be converted to EncodingError, but to
    // future proof.
    let error = match error.downcast::<CborError>() {
        Ok(enc_error) => {
            return Ok(CallError::new(
                ExitCode::ErrSerialization,
                enc_error.to_string(),
            ))
        }
        Err(other) => other,
    };

    // TODO @raulk these two blocks seem redundant in the presence of specific
    //  conversion adaptors for these types above.

    // Dynamic errors can come from Amt and Hamt through blockstore usages, check them.
    let error = match error.downcast::<AmtError>() {
        Ok(amt_err) => match *amt_err {
            AmtError::Dynamic(de) => match try_convert(de) {
                Ok(a) => return Ok(a),
                Err(other) => other,
            },
            other => Box::new(other),
        },
        Err(other) => other,
    };
    let error = match error.downcast::<HamtError>() {
        Ok(amt_err) => match *amt_err {
            HamtError::Dynamic(de) => match try_convert(de) {
                Ok(a) => return Ok(a),
                Err(other) => other,
            },
            other => Box::new(other),
        },
        Err(other) => other,
    };

    // Could not be converted automatically to actor error, return initial dynamic error.
    Err(error)
}
