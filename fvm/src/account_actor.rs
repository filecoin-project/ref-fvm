// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! This module contains the minimal logic for the FVM to handle account actor
//! auto-creation (on first transfer).
//!
//! ## Future direction
//!
//! This coupling between the FVM and a concrete actor must eventually be
//! eliminated. Refer to https://github.com/filecoin-project/fvm/issues/229 for
//! details.

use fvm_ipld_encoding::tuple::*;
use fvm_shared::address::Address;

/// State specifies the key address for the actor.
#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct State {
    pub address: Address,
}
