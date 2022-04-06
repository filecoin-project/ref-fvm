// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::{CborStoreError, Error as EncodingError};
use thiserror::Error;

/// HAMT Error
#[derive(Debug, Error)]
pub enum Error<BS: Blockstore> {
    #[error("hashbits: {0}")]
    HashBits(#[from] HashBitsError),
    /// This should be treated as a fatal error, must have at least one pointer in node
    #[error("Invalid HAMT format, node cannot have 0 pointers")]
    ZeroPointers,
    /// Cid not found in store error
    #[error("Cid ({0}) did not match any in database")]
    CidNotFound(String),
    #[error("blockstore {0}")]
    Blockstore(BS::Error),
    #[error("encoding error {0}")]
    Encoding(#[from] EncodingError),
}

impl<BS: Blockstore> From<CborStoreError<BS>> for Error<BS> {
    fn from(err: CborStoreError<BS>) -> Self {
        match err {
            CborStoreError::Blockstore(err) => Error::Blockstore(err),
            CborStoreError::Encoding(err) => Error::Encoding(err),
        }
    }
}

#[derive(Debug, Error)]
pub enum EitherError<U, BS: Blockstore> {
    #[error("user: {0}")]
    User(U),
    #[error("hamt: {0}")]
    Hamt(#[from] Error<BS>),
}

impl<U, BS: Blockstore> From<CborStoreError<BS>> for EitherError<U, BS> {
    fn from(err: CborStoreError<BS>) -> Self {
        EitherError::Hamt(err.into())
    }
}

#[derive(Error, Debug)]
pub enum HashBitsError {
    /// Maximum depth error
    #[error("Maximum depth reached")]
    MaxDepth,
    /// Hash bits does not support greater than 8 bit width
    #[error("HashBits does not support retrieving more than 8 bits")]
    InvalidLen,
}
