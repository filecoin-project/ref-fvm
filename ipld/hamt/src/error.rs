// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use fvm_shared::encoding::Error as EncodingError;
use std::error::Error as StdError;
use thiserror::Error;

/// HAMT Error
#[derive(Debug, Error)]
pub enum Error {
    /// Maximum depth error
    #[error("Maximum depth reached")]
    MaxDepth,
    /// Hash bits does not support greater than 8 bit width
    #[error("HashBits does not support retrieving more than 8 bits")]
    InvalidHashBitLen,
    /// This should be treated as a fatal error, must have at least one pointer in node
    #[error("Invalid HAMT format, node cannot have 0 pointers")]
    ZeroPointers,
    /// Cid not found in store error
    #[error("Cid ({0}) did not match any in database")]
    CidNotFound(String),
    // TODO: This should be something like "internal" or "io". And we shouldn't have both this and
    // "other"; they serve the same purpose.
    /// Dynamic error for when the error needs to be forwarded as is.
    #[error("{0}")]
    Dynamic(Box<dyn StdError>),
    /// Custom HAMT error
    #[error("{0}")]
    Other(String),
}

impl From<EncodingError> for Error {
    fn from(e: EncodingError) -> Self {
        Self::Dynamic(Box::new(e))
    }
}

impl From<Box<dyn StdError>> for Error {
    fn from(e: Box<dyn StdError>) -> Self {
        Self::Dynamic(e)
    }
}

impl From<Box<dyn StdError + Send + Sync + 'static>> for Error {
    fn from(e: Box<dyn StdError + Send + Sync + 'static>) -> Self {
        Self::Dynamic(e)
    }
}
