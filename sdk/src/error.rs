// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use thiserror::Error;

#[derive(Copy, Clone, Debug, Error, Eq, PartialEq)]
#[error("actor has been deleted")]
pub struct StateReadError;

#[derive(Copy, Clone, Debug, Error, Eq, PartialEq)]
pub enum StateUpdateError {
    #[error("actor has been deleted")]
    ActorDeleted,
    #[error("current execution context is read-only")]
    ReadOnly,
}

#[derive(Copy, Clone, Debug, Error, Eq, PartialEq)]
pub enum ActorDeleteError {
    #[error("cannot self-destruct when read-only")]
    ReadOnly,
    #[error("actor did not request unspent funds to be burnt")]
    UnspentFunds,
}

#[derive(Copy, Clone, Debug, Error, Eq, PartialEq)]
pub enum EpochBoundsError {
    #[error("the requested epoch isn't valid")]
    Invalid,
    #[error("the requested epoch exceeds the maximum lookback")]
    ExceedsLookback,
}
