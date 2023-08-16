// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use std::borrow::Cow;
use std::error;

use fvm_ipld_encoding::repr::*;
use fvm_ipld_encoding::{de, ser, strict_bytes, Error as EncodingError};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use thiserror::Error;

use crate::address::Error as AddressError;

/// BLS signature length in bytes.
pub const BLS_SIG_LEN: usize = 96;
/// BLS Public key length in bytes.
pub const BLS_PUB_LEN: usize = 48;
/// BLS message digest length in bytes (a compressed G2 affine point).
pub const BLS_DIGEST_LEN: usize = 96;

/// Secp256k1 signature length in bytes.
pub const SECP_SIG_LEN: usize = 65;
/// Secp256k1 Public key length in bytes.
pub const SECP_PUB_LEN: usize = 65;
/// Length of the signature input message hash in bytes (32).
pub const SECP_SIG_MESSAGE_HASH_SIZE: usize = 32;

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
    pub sig_type: SignatureType,
    pub bytes: Vec<u8>,
}

impl ser::Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let mut bytes = Vec::with_capacity(self.bytes.len() + 1);
        // Insert signature type byte
        bytes.push(self.sig_type as u8);
        bytes.extend_from_slice(&self.bytes);

        strict_bytes::Serialize::serialize(&bytes, serializer)
    }
}

impl<'de> de::Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let bytes: Cow<'de, [u8]> = strict_bytes::Deserialize::deserialize(deserializer)?;
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

#[cfg(feature = "arb")]
impl quickcheck::Arbitrary for SignatureType {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        if bool::arbitrary(g) {
            SignatureType::Secp256k1
        } else {
            SignatureType::BLS
        }
    }
}

#[cfg(feature = "arb")]
impl quickcheck::Arbitrary for Signature {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        Self {
            bytes: Vec::arbitrary(g),
            sig_type: SignatureType::arbitrary(g),
        }
    }
}

#[cfg(feature = "crypto")]
impl Signature {
    /// Checks if a signature is valid given data and address.
    pub fn verify(&self, digest: &[u8], addr: &crate::address::Address) -> Result<(), String> {
        verify(self.sig_type, &self.bytes, digest, addr)
    }
}

#[cfg(feature = "crypto")]
pub fn verify(
    sig_type: SignatureType,
    sig_data: &[u8],
    digest: &[u8],
    addr: &crate::address::Address,
) -> Result<(), String> {
    use crate::address::{Payload, Protocol};

    match sig_type {
        SignatureType::BLS => {
            let sig: &[u8; BLS_SIG_LEN] = sig_data.try_into().map_err(|_| {
                format!(
                    "invalid bls signature length {} (expected {BLS_SIG_LEN})",
                    sig_data.len()
                )
            })?;

            let pub_key = match addr.payload() {
                Payload::BLS(pub_key) => pub_key,
                addr_type => {
                    return Err(format!(
                        "cannot validate a BLS signature against a {} address",
                        Protocol::from(addr_type),
                    ))
                }
            };

            let digest: &[u8; BLS_DIGEST_LEN] = digest.try_into().map_err(|_| {
                format!(
                    "invalid bls digest length {} (expected {BLS_DIGEST_LEN})",
                    digest.len()
                )
            })?;

            if self::ops::verify_bls_aggregate(sig, &[pub_key], &[digest])? {
                Ok(())
            } else {
                Err(format!(
                    "bls signature verification failed for addr: {addr}"
                ))
            }
        }
        SignatureType::Secp256k1 => self::ops::verify_secp256k1_sig(sig_data, digest, addr),
    }
}

#[cfg(feature = "crypto")]
pub mod ops {
    use bls_signatures::{PublicKey as BlsPubKey, Serialize, Signature as BlsSignature};
    use libsecp256k1::{
        recover, Error as SecpError, Message, PublicKey, RecoveryId, Signature as EcsdaSignature,
    };

    use super::{
        Error, BLS_DIGEST_LEN, BLS_PUB_LEN, BLS_SIG_LEN, SECP_SIG_LEN, SECP_SIG_MESSAGE_HASH_SIZE,
    };
    use crate::address::{Address, Protocol};

    /// Verifies an aggregated BLS signature. Returns `Ok(false)` if signature verification fails
    /// and `String` error if arguments are invalid.
    pub fn verify_bls_aggregate(
        aggregate_sig: &[u8; BLS_SIG_LEN],
        pub_keys: &[&[u8; BLS_PUB_LEN]],
        digests: &[&[u8; BLS_DIGEST_LEN]],
    ) -> Result<bool, String> {
        // If the number of public keys and data does not match, return false;
        let (num_pub_keys, num_digests) = (pub_keys.len(), digests.len());
        if num_pub_keys != num_digests {
            return Err(format!(
                "unequal numbers of public keys ({num_pub_keys}) and digests ({num_digests})",
            ));
        }
        if num_pub_keys == 0 {
            return Ok(true);
        }

        // Deserialize signature bytes into a curve point.
        let sig = BlsSignature::from_bytes(aggregate_sig)
            .map_err(|_| "bls aggregate signature bytes are invalid G2 curve point".to_string())?;

        // Deserialize each public key's bytes into a curve point.
        let pub_keys = pub_keys
            .iter()
            .map(|pub_key| BlsPubKey::from_bytes(pub_key.as_slice()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "bls public key bytes are invalid G2 curve point".to_string())?;

        // Deserialize each digest's bytes into a curve point.
        //
        // BLS digests and signatures are each a G2 point. The `bls_signatures` crate does not
        // expose functionality for deserializing digest bytes into a G2 point, however it does
        // expose this serialization for its signature type `Signature`.
        let digests = {
            use bls_signatures::Signature as Digest;
            digests
                .iter()
                .map(|digest| Digest::from_bytes(digest.as_slice()).map(Into::into))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "bls digest bytes are invalid G2 curve point".to_string())?
        };

        Ok(bls_signatures::verify(&sig, &digests, &pub_keys))
    }

    /// Returns `String` error if a secp256k1 signature is invalid.
    pub fn verify_secp256k1_sig(
        signature: &[u8],
        hash: &[u8],
        addr: &Address,
    ) -> Result<(), String> {
        if addr.protocol() != Protocol::Secp256k1 {
            return Err(format!(
                "cannot validate a secp256k1 signature against a {} address",
                addr.protocol()
            ));
        }

        if signature.len() != SECP_SIG_LEN {
            return Err(format!(
                "Invalid Secp256k1 signature length. Was {}, must be 65",
                signature.len()
            ));
        }

        // Ecrecover with hash and signature
        let mut sig = [0u8; SECP_SIG_LEN];
        sig[..].copy_from_slice(signature);
        let rec_addr = ecrecover(hash.try_into().expect("fixed array size"), &sig)
            .map_err(|e| e.to_string())?;

        // check address against recovered address
        if &rec_addr == addr {
            Ok(())
        } else {
            Err("Secp signature verification failed".to_owned())
        }
    }

    /// Return the public key used for signing a message given it's signing bytes hash and signature.
    pub fn recover_secp_public_key(
        hash: &[u8; SECP_SIG_MESSAGE_HASH_SIZE],
        signature: &[u8; SECP_SIG_LEN],
    ) -> Result<PublicKey, Error> {
        // generate types to recover key from
        let rec_id = RecoveryId::parse(signature[64])?;
        let message = Message::parse(hash);

        // Signature value without recovery byte
        let mut s = [0u8; 64];
        s.clone_from_slice(signature[..64].as_ref());

        // generate Signature
        let sig = EcsdaSignature::parse_standard(&s)?;
        Ok(recover(&message, &sig, &rec_id)?)
    }

    /// Return Address for a message given it's signing bytes hash and signature.
    pub fn ecrecover(hash: &[u8; 32], signature: &[u8; SECP_SIG_LEN]) -> Result<Address, Error> {
        // recover public key from a message hash and secp signature.
        let key = recover_secp_public_key(hash, signature)?;
        let ret = key.serialize();
        let addr = Address::new_secp256k1(&ret)?;
        Ok(addr)
    }

    impl From<SecpError> for Error {
        fn from(err: SecpError) -> Error {
            match err {
                SecpError::InvalidRecoveryId => Error::InvalidRecovery(format!("{:?}", err)),
                _ => Error::SigningError(format!("{:?}", err)),
            }
        }
    }

    /// Hashes the plaintext of a Secp256k1 signature using the default hash function.
    pub fn hash_secp(data: &[u8]) -> [u8; SECP_SIG_MESSAGE_HASH_SIZE] {
        blake2b_simd::Params::new()
            .hash_length(32)
            .to_state()
            .update(data)
            .finalize()
            .as_bytes()
            .try_into()
            .expect("blake2b digest-to-array conversion should not fail")
    }

    /// Hashes the plaintext of a BLS signature using the default hash function.
    pub fn hash_bls(data: &[u8]) -> [u8; BLS_DIGEST_LEN] {
        bls_signatures::hash(data).to_compressed()
    }
}

#[cfg(all(test, feature = "crypto"))]
mod tests {
    use bls_signatures::{PrivateKey, Serialize, Signature as BlsSignature};
    use libsecp256k1::{sign, Message, PublicKey, SecretKey};
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    use super::ops::recover_secp_public_key;
    use super::*;
    use crate::crypto::signature::ops::{ecrecover, verify_bls_aggregate};
    use crate::Address;

    #[test]
    fn bls_agg_verify() {
        // The number of signatures in aggregate
        let num_sigs = 10;
        let message_length = num_sigs * 64;

        let rng = &mut ChaCha8Rng::seed_from_u64(11);

        let msg = (0..message_length).map(|_| rng.gen()).collect::<Vec<u8>>();
        let data: Vec<&[u8]> = (0..num_sigs).map(|x| &msg[x * 64..(x + 1) * 64]).collect();
        let digests: Vec<[u8; BLS_DIGEST_LEN]> = data
            .iter()
            .map(|msg| bls_signatures::hash(msg).to_compressed())
            .collect();
        let digests: Vec<&[u8; BLS_DIGEST_LEN]> = digests.iter().collect();

        let private_keys: Vec<PrivateKey> =
            (0..num_sigs).map(|_| PrivateKey::generate(rng)).collect();
        let public_keys: Vec<_> = private_keys
            .iter()
            .map(|x| x.public_key().as_bytes())
            .collect();

        let signatures: Vec<BlsSignature> = (0..num_sigs)
            .map(|x| private_keys[x].sign(data[x]))
            .collect();

        let public_keys_slice: Vec<&[u8; BLS_PUB_LEN]> = public_keys
            .iter()
            .map(|pub_key| {
                pub_key
                    .as_slice()
                    .try_into()
                    .expect("bls public key slice to array reference conversion should not fail")
            })
            .collect();

        let calculated_bls_agg = bls_signatures::aggregate(&signatures).unwrap().as_bytes();
        let agg_sig: &[u8; BLS_SIG_LEN] = calculated_bls_agg
            .as_slice()
            .try_into()
            .expect("bls signature slice to array reference conversion should not fail");

        assert_eq!(
            verify_bls_aggregate(agg_sig, &public_keys_slice, &digests),
            Ok(true)
        );
    }

    #[test]
    fn recover_pubkey() {
        let rng = &mut ChaCha8Rng::seed_from_u64(8);

        let privkey = SecretKey::random(rng);
        let pubkey = PublicKey::from_secret_key(&privkey);

        let hash: [u8; 32] = crate::crypto::signature::ops::hash_secp(&[42, 43]);

        // Generate signature
        let (sig, recovery_id) = sign(&Message::parse(&hash), &privkey);
        let mut signature = [0; 65];
        signature[..64].copy_from_slice(&sig.serialize());
        signature[64] = recovery_id.serialize();

        assert_eq!(pubkey, recover_secp_public_key(&hash, &signature).unwrap());
    }

    #[test]
    fn secp_ecrecover() {
        let rng = &mut ChaCha8Rng::seed_from_u64(8);

        let priv_key = SecretKey::random(rng);
        let pub_key = PublicKey::from_secret_key(&priv_key);
        let secp_addr = Address::new_secp256k1(&pub_key.serialize()).unwrap();

        let hash: [u8; 32] = blake2b_simd::Params::new()
            .hash_length(32)
            .to_state()
            .update(&[8, 8])
            .finalize()
            .as_bytes()
            .try_into()
            .expect("fixed array size");

        let msg = Message::parse(&hash);

        // Generate signature
        let (sig, recovery_id) = sign(&msg, &priv_key);
        let mut signature = [0; 65];
        signature[..64].copy_from_slice(&sig.serialize());
        signature[64] = recovery_id.serialize();

        assert_eq!(ecrecover(&hash, &signature).unwrap(), secp_addr);
    }
}

/// Crypto error
#[derive(Debug, PartialEq, Eq, Error)]
pub enum Error {
    /// Failed to produce a signature
    #[error("Failed to sign data {0}")]
    SigningError(String),
    /// Unable to perform ecrecover with the given params
    #[error("Could not recover public key from signature: {0}")]
    InvalidRecovery(String),
    /// Provided public key is not understood
    #[error("Invalid generated pub key to create address: {0}")]
    InvalidPubKey(#[from] AddressError),
}

impl From<Box<dyn error::Error>> for Error {
    fn from(err: Box<dyn error::Error>) -> Error {
        // Pass error encountered in signer trait as module error type
        Error::SigningError(err.to_string())
    }
}

impl From<EncodingError> for Error {
    fn from(err: EncodingError) -> Error {
        // Pass error encountered in signer trait as module error type
        Error::SigningError(err.to_string())
    }
}
