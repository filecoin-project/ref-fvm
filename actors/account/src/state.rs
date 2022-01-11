// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use fvm_shared::address::Address;
use fvm_shared::encoding::tuple::*;
use fvm_shared::encoding::Cbor;

/// State includes the address for the actor
#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct State {
    pub address: Address,
}

impl Cbor for State {}
