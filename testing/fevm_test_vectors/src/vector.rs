use cid::Cid;
use fvm_ipld_encoding::tuple::*;
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::receipt::Receipt;
use fvm_shared::ActorID;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Selector {
    #[serde(default)]
    pub chaos_actor: Option<String>,
    #[serde(default)]
    pub min_protocol_version: Option<String>,
    #[serde(default, rename = "requires:consensus_fault_extern")]
    pub consensus_fault: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MetaData {
    pub id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub comment: String,
    pub gen: Vec<GenerationData>,
    pub _debug: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GenerationData {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StateTreeVector {
    #[serde(with = "super::cidjson")]
    pub root_cid: Cid,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Variant {
    pub id: String,
    pub epoch: ChainEpoch,
    pub timestamp: Option<u64>,
    pub nv: u32,
}

/// Encoded VM randomness used to be replayed.
pub type Randomness = Vec<RandomnessMatch>;

/// One randomness entry.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RandomnessMatch {
    pub on: RandomnessRule,
    #[serde(with = "base64_bytes")]
    pub ret: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum RandomnessKind {
    Beacon,
    Chain,
}

/// Rule for matching when randomness is returned.
#[derive(Debug, Deserialize_tuple, Serialize_tuple, PartialEq, Eq, Clone)]
pub struct RandomnessRule {
    pub kind: RandomnessKind,
    pub dst: i64,
    pub epoch: ChainEpoch,
    #[serde(with = "base64_bytes")]
    pub entropy: Vec<u8>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TipsetCid {
    pub epoch: ChainEpoch,
    #[serde(with = "super::cidjson")]
    pub cid: Cid,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PreConditions {
    pub state_tree: StateTreeVector,
    #[serde(default)]
    pub basefee: Option<u128>,
    #[serde(default)]
    pub circ_supply: Option<u128>,
    #[serde(default)]
    pub variants: Vec<Variant>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PostConditions {
    pub state_tree: StateTreeVector,
    #[serde(with = "message_receipt_vec")]
    pub receipts: Vec<Receipt>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApplyMessage {
    #[serde(with = "base64_bytes")]
    pub bytes: Vec<u8>,
    #[serde(default)]
    pub epoch_offset: Option<ChainEpoch>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TestVector {
    pub class: String,

    pub chain_id: Option<u64>,

    pub selector: Option<Selector>,
    #[serde(rename = "_meta")]
    pub meta: Option<MetaData>,

    #[serde(with = "base64_bytes")]
    pub car: Vec<u8>,
    pub preconditions: PreConditions,
    pub apply_messages: Vec<ApplyMessage>,
    pub postconditions: PostConditions,
    #[serde(default)]
    pub randomness: Randomness,

    pub skip_compare_gas_used: bool,
    #[serde(with = "address_vec")]
    pub skip_compare_addresses: Option<Vec<Address>>,
    pub skip_compare_actor_ids: Option<Vec<ActorID>>,
    #[serde(with = "address_vec")]
    pub additional_compare_addresses: Option<Vec<Address>>,

    #[serde(default)]
    pub tipset_cids: Option<Vec<TipsetCid>>,
}

mod base64_bytes {
    use std::borrow::Cow;

    use serde::{de, Serializer};

    use super::*;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Cow<'de, str> = Deserialize::deserialize(deserializer)?;
        base64::decode(s.as_ref()).map_err(de::Error::custom)
    }

    pub fn serialize<S>(data: &Vec<u8>, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let encode_str = base64::encode(data);
        encode_str.serialize(serializer)
    }
}

mod message_receipt_vec {
    use fvm_ipld_encoding::RawBytes;
    use fvm_shared::error::ExitCode;
    use serde::Serializer;

    use super::*;

    #[derive(Deserialize, Serialize)]
    pub struct MessageReceiptVector {
        exit_code: ExitCode,
        #[serde(rename = "return", with = "base64_bytes")]
        return_value: Vec<u8>,
        gas_used: i64,
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Receipt>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Vec<MessageReceiptVector> = Deserialize::deserialize(deserializer)?;
        Ok(s.into_iter()
            .map(|v| Receipt {
                exit_code: v.exit_code,
                return_data: RawBytes::new(v.return_value),
                gas_used: v.gas_used,
                events_root: None,
            })
            .collect())
    }

    pub fn serialize<S>(data: &Vec<Receipt>, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let output: Vec<MessageReceiptVector> = data
            .into_iter()
            .map(|v| MessageReceiptVector {
                exit_code: v.exit_code,
                return_value: v.return_data.clone().into(),
                gas_used: v.gas_used,
            })
            .collect();
        output.serialize(serializer)
    }
}

mod address_vec {
    use std::str::FromStr;

    use serde::Serializer;

    use super::*;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<Address>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<Vec<String>> = Deserialize::deserialize(deserializer)?;
        if let Some(data) = s {
            let addr_strs: Vec<Address> = data
                .into_iter()
                .map(|v| Address::from_str(v.as_str()).unwrap())
                .collect();
            return Ok(Some(addr_strs));
        }
        Ok(None)
    }

    pub fn serialize<S>(
        data: &Option<Vec<Address>>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if let Some(addrs) = data {
            let output: Vec<String> = addrs.into_iter().map(|v| v.to_string()).collect();
            return Some(output).serialize(serializer);
        }
        return serializer.serialize_none();
    }
}
