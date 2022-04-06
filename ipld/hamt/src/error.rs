// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::error::Error as StdError;

use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::{CborStore, CborStoreError, Error as EncodingError};
use thiserror::Error;

/// HAMT Error
#[derive(Debug, Error)]
pub enum Error<BS: Blockstore> {
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
    #[error("blockstore {0}")]
    Blockstore(BS::Error),
    #[error("encoding error {0}")]
    Encoding(#[from] EncodingError),
}
