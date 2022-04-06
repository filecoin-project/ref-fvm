// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use cid::Error as CidError;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::{CborStoreError, Error as EncodingError};
use thiserror::Error;

/// AMT Error
#[derive(Debug, Error)]
pub enum Error<BS: Blockstore> {
    /// Index referenced it above arbitrary max set
    #[error("index {0} out of range for the amt")]
    OutOfRange(u64),
    /// Height of root node is greater than max.
    #[error("failed to load AMT: height out of bounds: {0} > {1}")]
    MaxHeight(u32, u32),
    /// Error generating a Cid for data
    #[error(transparent)]
    Cid(#[from] CidError),
    /// Serialized vector less than number of bits set
    #[error("Vector length does not match bitmap")]
    InvalidVecLength,
    /// Cid not found in store error
    #[error("Cid ({0}) did not match any in database")]
    CidNotFound(String),
    #[error("{0}")]
    CollapsedNode(#[from] CollapsedNodeError),
    #[error("no such index {0} in Amt for batch delete")]
    BatchDelteNotFound(u64),
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

#[derive(Debug, Error)]
pub enum CollapsedNodeError {
    #[error("expected bitfield of length {0}, found bitfield with length {1}")]
    LengthMissmatch(usize, usize),
    #[error("Bitmap contained more set bits than links provided")]
    MoreBitsThanLinks,
    #[error("Bitmap contained less set bits than links provided")]
    LessBitsThanLinks,
    #[error("Bitmap contained more set bits than values provided")]
    MoreBitsThanValues,
    #[error("Bitmap contained less set bits than values provided")]
    LessBitsThanValues,
    /// Invalid formatted serialized node.
    #[error("Serialized node cannot contain both links and values")]
    LinksAndValues,
}

#[derive(Debug, Error)]
pub enum SerdeError {
    /// Error when trying to serialize an AMT without a flushed cache
    #[error("Tried to serialize without saving cache, run flush() on Amt before serializing")]
    Cached,
}
