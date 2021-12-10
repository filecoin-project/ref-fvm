// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use crate::address::{Address, Error as AddressError, Protocol};
use crate::encoding::{blake2b_256, de, repr::*, ser, serde_bytes, Cbor, Error as EncodingError};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use std::borrow::Cow;
use std::error;
use thiserror::Error;

/// BLS signature length in bytes.
pub const BLS_SIG_LEN: usize = 96;
/// BLS Public key length in bytes.
pub const BLS_PUB_LEN: usize = 48;

/// Secp256k1 signature length in bytes.
pub const SECP_SIG_LEN: usize = 65;
/// Secp256k1 Public key length in bytes.
pub const SECP_PUB_LEN: usize = 65;

/// Signature variants for Filecoin signatures.
#[derive(
    Clone, Debug, PartialEq, FromPrimitive, Copy, Eq, Serialize_repr, Deserialize_repr, Hash,
)]
#[repr(u8)]
pub enum SignatureType {
    Secp256k1 = 1,
    BLS = 2,
}

/// A cryptographic signature, represented in bytes, of any key protocol.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Signature {
    sig_type: SignatureType,
    bytes: Vec<u8>,
}

impl Cbor for Signature {}

impl ser::Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let mut bytes = Vec::with_capacity(self.bytes.len() + 1);
        // Insert signature type byte
        bytes.push(self.sig_type as u8);
        bytes.extend_from_slice(&self.bytes);

        serde_bytes::Serialize::serialize(&bytes, serializer)
    }
}

impl<'de> de::Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let bytes: Cow<'de, [u8]> = serde_bytes::Deserialize::deserialize(deserializer)?;
        if bytes.is_empty() {
            return Err(de::Error::custom("Cannot deserialize empty bytes"));
        }

        // Remove signature type byte
        let sig_type = SignatureType::from_u8(bytes[0])
            .ok_or_else(|| de::Error::custom("Invalid signature type byte (must be 1 or 2)"))?;

        Ok(Signature {
            bytes: bytes[1..].to_vec(),
            sig_type,
        })
    }
}

impl Signature {
    /// Creates a SECP Signature given the raw bytes.
    pub fn new_secp256k1(bytes: Vec<u8>) -> Self {
        Self {
            sig_type: SignatureType::Secp256k1,
            bytes,
        }
    }

    /// Creates a BLS Signature given the raw bytes.
    pub fn new_bls(bytes: Vec<u8>) -> Self {
        Self {
            sig_type: SignatureType::BLS,
            bytes,
        }
    }

    /// Returns reference to signature bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Returns [SignatureType] for the signature.
    pub fn signature_type(&self) -> SignatureType {
        self.sig_type
    }
}

#[cfg(feature = "json")]
pub mod json {
    use super::*;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    // Wrapper for serializing and deserializing a Signature from JSON.
    #[derive(Deserialize, Serialize)]
    #[serde(transparent)]
    pub struct SignatureJson(#[serde(with = "self")] pub Signature);

    /// Wrapper for serializing a Signature reference to JSON.
    #[derive(Serialize)]
    #[serde(transparent)]
    pub struct SignatureJsonRef<'a>(#[serde(with = "self")] pub &'a Signature);

    #[derive(Serialize, Deserialize)]
    struct JsonHelper {
        #[serde(rename = "Type")]
        sig_type: SignatureType,
        #[serde(rename = "Data")]
        bytes: String,
    }

    pub fn serialize<S>(m: &Signature, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        JsonHelper {
            sig_type: m.sig_type,
            bytes: base64::encode(&m.bytes),
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Signature, D::Error>
    where
        D: Deserializer<'de>,
    {
        let JsonHelper { sig_type, bytes } = Deserialize::deserialize(deserializer)?;
        Ok(Signature {
            sig_type,
            bytes: base64::decode(bytes).map_err(de::Error::custom)?,
        })
    }

    pub mod opt {
        use super::{Signature, SignatureJson, SignatureJsonRef};
        use serde::{self, Deserialize, Deserializer, Serialize, Serializer};

        pub fn serialize<S>(v: &Option<Signature>, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            v.as_ref()
                .map(|s| SignatureJsonRef(s))
                .serialize(serializer)
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Signature>, D::Error>
        where
            D: Deserializer<'de>,
        {
            let s: Option<SignatureJson> = Deserialize::deserialize(deserializer)?;
            Ok(s.map(|v| v.0))
        }
    }

    pub mod signature_type {
        use super::*;
        use serde::{Deserialize, Deserializer, Serialize, Serializer};

        #[derive(Debug, Deserialize, Serialize)]
        #[serde(rename_all = "lowercase")]
        enum JsonHelperEnum {
            Bls,
            Secp256k1,
        }

        #[derive(Debug, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct SignatureTypeJson(#[serde(with = "self")] pub SignatureType);

        pub fn serialize<S>(m: &SignatureType, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let json = match m {
                SignatureType::BLS => JsonHelperEnum::Bls,
                SignatureType::Secp256k1 => JsonHelperEnum::Secp256k1,
            };
            json.serialize(serializer)
        }

        pub fn deserialize<'de, D>(deserializer: D) -> Result<SignatureType, D::Error>
        where
            D: Deserializer<'de>,
        {
            let json_enum: JsonHelperEnum = Deserialize::deserialize(deserializer)?;

            let signature_type = match json_enum {
                JsonHelperEnum::Bls => SignatureType::BLS,
                JsonHelperEnum::Secp256k1 => SignatureType::Secp256k1,
            };
            Ok(signature_type)
        }
    }
}
