// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::{strict_bytes, Cbor};

use crate::address::Address;
use crate::econ::TokenAmount;
use crate::MethodNum;

/// Default Unsigned VM message type which includes all data needed for a state transition
#[cfg_attr(feature = "testing", derive(Default))]
#[derive(PartialEq, Clone, Debug, Hash, Eq, Serialize_tuple, Deserialize_tuple)]
pub struct Message {
    pub version: i64,

    pub to: Address,
    pub from: Address,

    pub sequence: u64,

    pub value: TokenAmount,

    pub gas_limit: i64,
    pub gas_fee_cap: TokenAmount,
    pub gas_premium: TokenAmount,

    pub method_num: MethodNum,
    #[serde(with = "strict_bytes")]
    pub params: Vec<u8>,
}

impl Cbor for Message {}

impl Message {
    /// Does some basic checks on the Message to see if the fields are valid.
    pub fn check(self: &Message) -> anyhow::Result<()> {
        if self.gas_limit == 0 {
            return Err(anyhow!("Message has no gas limit set"));
        }
        if self.gas_limit < 0 {
            return Err(anyhow!("Message has negative gas limit"));
        }
        Ok(())
    }
}
