// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::Cid;

#[derive(thiserror::Error, Debug)]
/// Util errors for the intergration test framework.
pub(crate) enum Error {
    #[error("could not find manifest information for cid: {0}")]
    NoManifestInformation(Cid),
    #[error("could not load builtin manifest")]
    FailedToLoadManifest,
    #[error("could not set state in tree for: {0}")]
    FailedToSetState(String),
    #[error("failed to flush tree")]
    FailedToFlushTree,
}
