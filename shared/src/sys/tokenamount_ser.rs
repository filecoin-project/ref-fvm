// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::borrow::Cow;

use num_bigint::{BigInt, Sign};
use serde::{Deserialize, Serialize};

use crate::sys::TokenAmount;

/// Wrapper for serializing token amounts to match filecoin spec. Serializes as bytes.
#[derive(Serialize)]
#[serde(transparent)]
pub struct TokenAmountSer<'a>(#[serde(with = "self")] pub &'a TokenAmount);

/// Wrapper for deserializing as TokenAmount from bytes.
#[derive(Deserialize, Serialize, Clone, Default, PartialEq)]
#[serde(transparent)]
pub struct TokenAmountDe(#[serde(with = "self")] pub TokenAmount);

/// Serializes TokenAmount as bytes following Filecoin spec.
pub fn serialize<S>(token: &TokenAmount, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    // review note: safe to deref? should we have a second impl of from with pointer rcvr?
    let int = crate::econ::TokenAmount::from(*token);
    // let int: crate::econ::TokenAmount = token.into();
    let (sign, mut bz) = int.to_bytes_be();

    // Insert sign byte at start of encoded bytes
    match sign {
        Sign::Minus => bz.insert(0, 1),
        Sign::Plus => bz.insert(0, 0),
        Sign::NoSign => bz = Vec::new(),
    }

    // Serialize as bytes
    serde_bytes::Serialize::serialize(&bz, serializer)
}

/// Deserializes bytes into TokenAmount
pub fn deserialize<'de, D>(deserializer: D) -> Result<TokenAmount, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bz: Cow<'de, [u8]> = serde_bytes::Deserialize::deserialize(deserializer)?;
    if bz.is_empty() {
        return Ok(TokenAmount::default());
    }
    let sign_byte = bz[0];
    let sign: Sign = match sign_byte {
        1 => Sign::Minus,
        0 => Sign::Plus,
        _ => {
            return Err(serde::de::Error::custom(
                "First byte must be valid sign (0, 1)",
            ));
        }
    };
    Ok(BigInt::from_bytes_be(sign, &bz[1..]).try_into().unwrap())
}
