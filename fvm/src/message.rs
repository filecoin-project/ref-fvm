// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use derive_builder::Builder;

use fvm_shared::address::Address;
use fvm_shared::bigint::bigint_ser::{BigIntDe, BigIntSer};
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{
    de::{Deserialize, Deserializer},
    ser::{Serialize, Serializer},
    Cbor, RawBytes,
};
use fvm_shared::MethodNum;

use crate::kernel::{ClassifyResult, Result};

/// Default Unsigned VM message type which includes all data needed for a state transition
#[derive(PartialEq, Clone, Debug, Hash, Eq, Builder)]
pub struct Message {
    pub version: i64,
    pub from: Address,
    pub to: Address,
    pub sequence: u64,
    pub value: TokenAmount,
    pub method_num: MethodNum,
    pub params: RawBytes,
    pub gas_limit: i64,
    pub gas_fee_cap: TokenAmount,
    pub gas_premium: TokenAmount,
}

impl Message {
    /// Helper function to convert the message into signing bytes.
    /// This function returns the message `Cid` bytes.
    pub fn to_signing_bytes(&self) -> Vec<u8> {
        // Safe to unwrap here, unsigned message cannot fail to serialize.
        self.cid().unwrap().to_bytes()
    }

    /// Does some basic checks on the Message to see if the fields are valid.
    pub fn check(self: &Message) -> Result<()> {
        if self.gas_limit == 0 {
            return Err(anyhow!("Message has no gas limit set")).or_fatal();
        }
        if self.gas_limit < 0 {
            return Err(anyhow!("Message has negative gas limit")).or_fatal();
        }
        Ok(())
    }
}

impl Serialize for Message {
    fn serialize<S>(&self, s: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        (
            &self.version,
            &self.to,
            &self.from,
            &self.sequence,
            BigIntSer(&self.value),
            &self.gas_limit,
            BigIntSer(&self.gas_fee_cap),
            BigIntSer(&self.gas_premium),
            &self.method_num,
            &self.params,
        )
            .serialize(s)
    }
}

impl<'de> Deserialize<'de> for Message {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let (
            version,
            to,
            from,
            sequence,
            BigIntDe(value),
            gas_limit,
            BigIntDe(gas_fee_cap),
            BigIntDe(gas_premium),
            method_num,
            params,
        ) = Deserialize::deserialize(deserializer)?;
        Ok(Self {
            version,
            from,
            to,
            sequence,
            value,
            method_num,
            params,
            gas_limit,
            gas_fee_cap,
            gas_premium,
        })
    }
}

impl Cbor for Message {}
