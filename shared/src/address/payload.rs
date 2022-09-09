// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::convert::TryInto;
use std::hash::Hash;
use std::ops::Deref;
use std::u64;

use super::{
    from_leb_bytes, to_leb_bytes, Error, Protocol, BLS_PUB_LEN, MAX_SUBADDRESS_LEN,
    PAYLOAD_HASH_LEN,
};
use crate::ActorID;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Subaddress {
    len: u8,
    buf: [u8; MAX_SUBADDRESS_LEN],
}

#[cfg(feature = "arb")]
impl<'a> arbitrary::Arbitrary<'a> for Subaddress {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Subaddress {
            len: u.int_in_range(0u8..=(MAX_SUBADDRESS_LEN as u8))?,
            buf: arbitrary::Arbitrary::arbitrary(u)?,
        })
    }
}

impl AsRef<[u8]> for Subaddress {
    fn as_ref(&self) -> &[u8] {
        &self.buf[..self.len as usize]
    }
}

impl Deref for Subaddress {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl<'a> TryFrom<&'a [u8]> for Subaddress {
    type Error = Error;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        let length = value.len();
        if length > MAX_SUBADDRESS_LEN {
            return Err(Error::InvalidPayloadLength(length));
        }
        let mut addr = Subaddress {
            len: length as u8,
            buf: [0u8; MAX_SUBADDRESS_LEN],
        };
        addr.buf[..length].copy_from_slice(&value[..length]);
        Ok(addr)
    }
}

/// Payload is the data of the Address. Variants are the supported Address protocols.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
#[cfg_attr(feature = "arb", derive(arbitrary::Arbitrary))]
pub enum Payload {
    /// ID protocol address.
    ID(u64),
    /// SECP256K1 key address, 20 byte hash of PublicKey
    Secp256k1([u8; PAYLOAD_HASH_LEN]),
    /// Actor protocol address, 20 byte hash of actor data
    Actor([u8; PAYLOAD_HASH_LEN]),
    /// BLS key address, full 48 byte public key
    BLS([u8; BLS_PUB_LEN]),
    /// Namespaced
    Namespaced {
        namespace: ActorID,
        subaddress: Subaddress,
    },
}

impl Payload {
    /// Returns encoded bytes of Address without the protocol byte.
    pub fn to_raw_bytes(self) -> Vec<u8> {
        use Payload::*;
        match self {
            ID(i) => to_leb_bytes(i),
            Secp256k1(arr) => arr.to_vec(),
            Actor(arr) => arr.to_vec(),
            BLS(arr) => arr.to_vec(),
            Namespaced {
                namespace,
                subaddress,
            } => {
                let mut buf = to_leb_bytes(namespace);
                buf.extend(&*subaddress);
                buf
            }
        }
    }

    /// Returns encoded bytes of Address including the protocol byte.
    pub fn to_bytes(self) -> Vec<u8> {
        let mut bz = self.to_raw_bytes();
        bz.insert(0, Protocol::from(self) as u8);
        bz
    }

    /// Generates payload from raw bytes and protocol.
    pub fn new(protocol: Protocol, payload: &[u8]) -> Result<Self, Error> {
        let payload = match protocol {
            Protocol::ID => Self::ID(from_leb_bytes(payload)?),
            Protocol::Secp256k1 => Self::Secp256k1(
                payload
                    .try_into()
                    .map_err(|_| Error::InvalidPayloadLength(payload.len()))?,
            ),
            Protocol::Actor => Self::Actor(
                payload
                    .try_into()
                    .map_err(|_| Error::InvalidPayloadLength(payload.len()))?,
            ),
            Protocol::BLS => Self::BLS(
                payload
                    .try_into()
                    .map_err(|_| Error::InvalidPayloadLength(payload.len()))?,
            ),
            Protocol::Namespaced => {
                let (id, remaining) = unsigned_varint::decode::u64(payload)?;
                Self::Namespaced {
                    namespace: id,
                    subaddress: remaining.try_into()?,
                }
            }
        };
        Ok(payload)
    }
}

impl From<Payload> for Protocol {
    fn from(pl: Payload) -> Self {
        match pl {
            Payload::ID(_) => Self::ID,
            Payload::Secp256k1(_) => Self::Secp256k1,
            Payload::Actor(_) => Self::Actor,
            Payload::BLS(_) => Self::BLS,
            Payload::Namespaced { .. } => Self::Namespaced,
        }
    }
}

impl From<&Payload> for Protocol {
    fn from(pl: &Payload) -> Self {
        match pl {
            Payload::ID(_) => Self::ID,
            Payload::Secp256k1(_) => Self::Secp256k1,
            Payload::Actor(_) => Self::Actor,
            Payload::BLS(_) => Self::BLS,
            Payload::Namespaced { .. } => Self::Namespaced,
        }
    }
}

#[cfg(feature = "testing")]
impl Default for Payload {
    fn default() -> Self {
        Payload::ID(0)
    }
}
