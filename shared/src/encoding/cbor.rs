// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::ops::Deref;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::errors::Error;
use crate::encoding::{from_slice, to_vec, CodecProtocol};
use cid::{multihash, Cid};

// TODO find something to reference.
pub const DAG_CBOR: u64 = 0x71;

/// Cbor utility functions for serializable objects
pub trait Cbor: Serialize + DeserializeOwned {
    /// Marshalls cbor encodable object into cbor bytes
    fn marshal_cbor(&self) -> Result<Vec<u8>, Error> {
        Ok(to_vec(&self)?)
    }

    /// Unmarshals cbor encoded bytes to object
    fn unmarshal_cbor(bz: &[u8]) -> Result<Self, Error> {
        Ok(from_slice(bz)?)
    }

    /// Returns the content identifier of the raw block of data
    /// Default is Blake2b256 hash
    fn cid(&self) -> Result<Cid, Error> {
        use multihash::MultihashDigest;
        const DIGEST_SIZE: u32 = 32; // TODO get from the multihash?
        let data = &self.marshal_cbor()?;
        let hash = multihash::Code::Blake2b256.digest(data);
        if u32::from(hash.size()) != DIGEST_SIZE {
            return Err(Error {
                description: "Invalid multihash length".into(),
                protocol: CodecProtocol::Cbor, // TODO this is not accurate, and not convinced about this Error type.
            });
        }
        Ok(Cid::new_v1(DAG_CBOR, hash))
    }
}

impl<T> Cbor for Vec<T> where T: Cbor {}
impl<T> Cbor for Option<T> where T: Cbor {}

/// Raw serialized cbor bytes.
/// This data is (de)serialized as a byte string.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Hash, Eq, Default)]
#[serde(transparent)]
pub struct RawBytes {
    #[serde(with = "serde_bytes")]
    bytes: Vec<u8>,
}

impl Cbor for RawBytes {}

impl Deref for RawBytes {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

impl RawBytes {
    /// Constructor if data is encoded already
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Contructor for encoding Cbor encodable structure.
    pub fn serialize<O: Serialize>(obj: O) -> Result<Self, Error> {
        Ok(Self {
            bytes: to_vec(&obj)?,
        })
    }

    /// Returns serialized bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Deserializes the serialized bytes into a defined type.
    pub fn deserialize<O: DeserializeOwned>(&self) -> Result<O, Error> {
        Ok(from_slice(&self.bytes)?)
    }
}
