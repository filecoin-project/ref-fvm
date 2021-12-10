// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use encoding::error::Error as CborError;
use serde::ser;
use std::fmt;
use thiserror::Error;

/// Ipld error
#[derive(Debug, PartialEq, Error)]
pub enum IpldError {
    #[error("{0}")]
    Encoding(String),
    #[error("{0}")]
    Other(&'static str),
    #[error("Failed to traverse link: {0}")]
    Link(String),
    #[error("{0}")]
    Custom(String),
}

impl ser::Error for IpldError {
    fn custom<T: fmt::Display>(msg: T) -> IpldError {
        IpldError::Encoding(msg.to_string())
    }
}

impl From<CborError> for IpldError {
    fn from(e: CborError) -> IpldError {
        IpldError::Encoding(e.to_string())
    }
}
