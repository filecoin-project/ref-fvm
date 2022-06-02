// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::pin::Pin;
use std::task::{Context, Poll};

use anyhow::{anyhow, Context as _};
use cid::Cid;
use flate2::bufread::GzDecoder;
use futures::AsyncRead;
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_car::load_car;
use fvm_ipld_encoding::tuple::*;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::receipt::Receipt;
use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize, Clone)]
pub struct StateTreeVector {
    #[serde(with = "super::cidjson")]
    pub root_cid: Cid,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GenerationData {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MetaData {
    pub id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub comment: String,
    pub gen: Vec<GenerationData>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PreConditions {
    pub state_tree: StateTreeVector,
    #[serde(default)]
    pub basefee: Option<u128>,
    #[serde(default)]
    pub circ_supply: Option<u128>,
    #[serde(default)]
    pub variants: Vec<Variant>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PostConditions {
    pub state_tree: StateTreeVector,
    #[serde(with = "message_receipt_vec")]
    pub receipts: Vec<Receipt>,
    #[serde(default, with = "super::cidjson::vec")]
    pub receipts_roots: Vec<Cid>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Selector {
    #[serde(default)]
    pub chaos_actor: Option<String>,
    #[serde(default)]
    pub min_protocol_version: Option<String>,
    #[serde(default, rename = "requires:consensus_fault_extern")]
    pub consensus_fault: Option<String>,
}

impl Selector {
    /// Returns whether this runner supports applying vectors with this selector.
    pub fn supported(&self) -> bool {
        self.chaos_actor.as_deref() != Some("true")
            && self.consensus_fault.as_deref() != Some("true")
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Variant {
    pub id: String,
    pub epoch: ChainEpoch,
    pub nv: u32,
}

/// Encoded VM randomness used to be replayed.
pub type Randomness = Vec<RandomnessMatch>;

/// One randomness entry.
#[derive(Debug, Deserialize, Clone)]
pub struct RandomnessMatch {
    pub on: RandomnessRule,
    #[serde(with = "base64_bytes")]
    pub ret: Vec<u8>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum RandomnessKind {
    Beacon,
    Chain,
}

/// Rule for matching when randomness is returned.
#[derive(Debug, Deserialize_tuple, PartialEq, Clone)]
pub struct RandomnessRule {
    pub kind: RandomnessKind,
    pub dst: i64,
    pub epoch: ChainEpoch,
    #[serde(with = "base64_bytes")]
    pub entropy: Vec<u8>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MessageVector {
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
}

impl MessageVector {
    /// Loads a message vector from a file.
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        // Test vectors have the form:
        //
        //     { "class": ..., rest... }
        //
        // Unfortunately:
        // 1. That means we need to use serde's "flatten" and/or "tag" feature to decode them.
        // 2. Serde's JSON library doesn't support arbitrary precision numbers when doing this.
        // 3. The circulating supply doesn't fit in a u64, and f64 isn't precise enough.
        //
        // So we manually:
        // 1. Decode into a map of `String` -> `raw data`.
        // 2. Pull off the class.
        // 3. Re-serialize.
        // 4. Decode into the correct type.
        //
        // Upstream bug is https://github.com/serde-rs/serde/issues/1183 (or at least that looks like
        // the most appropriate one out of all the related issues).
        let mut vector: HashMap<String, Box<serde_json::value::RawValue>> =
            serde_json::from_reader(reader)?;

        let class_json = vector
            .remove("class")
            .context("expected test vector to have a class")?;

        let class: &str = serde_json::from_str(class_json.get())?;
        if class != "message" {
            return Err(anyhow!("unknown test vector class: {}", class));
        }

        let vector_json = serde_json::to_string(&vector)?;
        Ok(serde_json::from_str(&vector_json)?)
    }

    /// Returns true if the vector is supported.
    pub fn is_supported(&self) -> bool {
        self.selector.as_ref().map_or(true, Selector::supported)
    }
}

impl MessageVector {
    /// Seeds a new blockstore with the CAR encoded in the test vector, and
    /// returns the blockstore and the root CID.
    pub async fn seed_blockstore(&self) -> anyhow::Result<(MemoryBlockstore, Vec<Cid>)> {
        let blockstore = MemoryBlockstore::new();
        let bytes = self.car.as_slice();
        let decoder = GzipDecoder(GzDecoder::new(bytes));
        let cid = load_car(&blockstore, decoder).await?;
        Ok((blockstore, cid))
    }
}

struct GzipDecoder<R>(GzDecoder<R>);

impl<R: std::io::Read + Unpin + std::io::BufRead> AsyncRead for GzipDecoder<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        Poll::Ready(std::io::Read::read(&mut self.0, buf))
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApplyMessage {
    #[serde(with = "base64_bytes")]
    pub bytes: Vec<u8>,
    #[serde(default)]
    pub epoch_offset: Option<ChainEpoch>,
}

mod base64_bytes {
    use std::borrow::Cow;

    use serde::de;

    use super::*;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Cow<'de, str> = Deserialize::deserialize(deserializer)?;
        base64::decode(s.as_ref()).map_err(de::Error::custom)
    }
}

mod message_receipt_vec {
    use fvm_ipld_encoding::RawBytes;
    use fvm_shared::error::ExitCode;

    use super::*;

    #[derive(Deserialize)]
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
            })
            .collect())
    }
}

// // This might be changed to be encoded into vector, matching go runner for now
// pub fn to_chain_msg(msg: UnsignedMessage) -> ChainMessage {
//     if msg.from().protocol() == Protocol::Secp256k1 {
//         ChainMessage::Signed(SignedMessage {
//             message: msg,
//             signature: Signature::new_secp256k1(vec![0; 65]),
//         })
//     } else {
//         ChainMessage::Unsigned(msg)
//     }
// }
