// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use thiserror::Error;

#[derive(PartialEq, Eq, Clone, Debug, Error)]
pub enum Error {
    #[error("bitfield not minimally encoded")]
    NotMinimal,
    #[error("bitfield specifies an unsupported version")]
    UnsupportedVersion,
    #[error("bitfield overflows 2^63-1")]
    RLEOverflow,
    #[error("invalid varint")]
    InvalidVarint,
}
