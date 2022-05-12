// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::convert::TryFrom;

use fvm_ipld_encoding::serde_bytes;
use serde::{Deserialize, Deserializer, Serialize};

use super::BitField;
use crate::{Error, MAX_ENCODED_SIZE};

/// A trait for types that can produce a `&BitField` (or fail to do so).
/// Generalizes over `&BitField` and `&mut UnvalidatedBitField`.
pub trait Validate<'a> {
    fn validate(self) -> Result<&'a BitField, Error>;
}

impl<'a> Validate<'a> for &'a mut UnvalidatedBitField {
    /// Validates the RLE+ encoding of the bit field, returning a shared
    /// reference to the decoded bit field.
    fn validate(self) -> Result<&'a BitField, Error> {
        self.validate_mut().map(|bf| &*bf)
    }
}

impl<'a> Validate<'a> for &'a BitField {
    fn validate(self) -> Result<&'a BitField, Error> {
        Ok(self)
    }
}

/// A bit field that may not yet have been validated for valid RLE+.
/// Used to defer this validation step until when the bit field is
/// first used, rather than at deserialization.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum UnvalidatedBitField {
    Validated(BitField),
    Unvalidated(#[serde(with = "serde_bytes")] Vec<u8>),
}

impl UnvalidatedBitField {
    /// Validates the RLE+ encoding of the bit field, returning a unique
    /// reference to the decoded bit field.
    pub fn validate_mut(&mut self) -> Result<&mut BitField, Error> {
        if let Self::Unvalidated(bytes) = self {
            *self = Self::Validated(BitField::from_bytes(bytes)?);
        }

        match self {
            Self::Validated(bf) => Ok(bf),
            Self::Unvalidated(_) => unreachable!(),
        }
    }
}

impl From<BitField> for UnvalidatedBitField {
    fn from(bf: BitField) -> Self {
        Self::Validated(bf)
    }
}

impl TryFrom<UnvalidatedBitField> for BitField {
    type Error = Error;

    fn try_from(bf: UnvalidatedBitField) -> Result<Self, Self::Error> {
        match bf {
            UnvalidatedBitField::Validated(bf) => Ok(bf),
            UnvalidatedBitField::Unvalidated(bf) => BitField::from_bytes(&bf),
        }
    }
}

impl<'de> Deserialize<'de> for UnvalidatedBitField {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = serde_bytes::deserialize(deserializer)?;
        if bytes.len() > MAX_ENCODED_SIZE {
            return Err(serde::de::Error::custom(format!(
                "encoded bitfield was too large {}",
                bytes.len()
            )));
        }
        Ok(Self::Unvalidated(bytes))
    }
}
