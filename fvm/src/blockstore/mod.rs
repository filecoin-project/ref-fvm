// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! Private blockstores for use in the FVM.

mod buffered;
mod discard;

pub use buffered::BufferedBlockstore;
pub(crate) use discard::DiscardBlockstore;
