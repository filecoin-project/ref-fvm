// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::anyhow;
use fvm_ipld_encoding::de::{Deserialize, Deserializer};
use fvm_ipld_encoding::ser::{Serialize, Serializer};
use fvm_ipld_encoding::{Cbor, RawBytes};

use crate::address::Address;
use crate::econ::TokenAmount;
use crate::MethodNum;

/// Default Unsigned VM message type which includes all data needed for a state transition
#[cfg_attr(feature = "testing", derive(Default))]
#[derive(PartialEq, Clone, Debug, Hash, Eq)]
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
            &self.value,
            &self.gas_limit,
            &self.gas_fee_cap,
            &self.gas_premium,
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
            value,
            gas_limit,
            gas_fee_cap,
            gas_premium,
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

pub mod params {
    use fvm_ipld_encoding::strict_bytes;
    use serde_tuple::{Deserialize_tuple, Serialize_tuple};

    use super::{Cbor, Message};

    /// Params of the `validate` entrypoint, signature raw bytes and the whole filecoin message to be validated
    #[derive(Debug, Serialize_tuple, Deserialize_tuple)]
    pub struct ValidateParams {
        #[serde(with = "strict_bytes")]
        pub signature: Vec<u8>,
        pub message: Message,
    }

    impl ValidateParams {
        pub fn new(msg: Message, sig: Vec<u8>) -> Self {
            Self {
                signature: sig,
                message: msg,
            }
        }
    }

    impl Cbor for ValidateParams {}
}
