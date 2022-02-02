// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use fvm_shared::address::Address;
use fvm_shared::bigint::bigint_ser;
use fvm_shared::crypto::signature::Signature;
use fvm_shared::encoding::tuple::*;
use fvm_shared::sector::StoragePower;
use lazy_static::lazy_static;
use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};

#[cfg(not(feature = "devnet"))]
lazy_static! {
    pub static ref MINIMUM_VERIFIED_DEAL_SIZE: StoragePower = StoragePower::from_i32(1 << 20).unwrap(); // placeholder
}

#[cfg(feature = "devnet")]
lazy_static! {
    pub static ref MINIMUM_VERIFIED_DEAL_SIZE: StoragePower = StoragePower::from_i32(256).unwrap(); // placeholder
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize_tuple, Deserialize_tuple)]
pub struct VerifierParams {
    pub address: Address,
    #[serde(with = "bigint_ser")]
    pub allowance: DataCap,
}

pub type AddVerifierParams = VerifierParams;
pub type AddVerifierClientParams = VerifierParams;

/// DataCap is an integer number of bytes.
/// We can introduce policy changes and replace this in the future.
pub type DataCap = StoragePower;

#[derive(Clone, Debug, PartialEq, Serialize_tuple, Deserialize_tuple)]
pub struct BytesParams {
    /// Address of verified client.
    pub address: Address,
    /// Number of bytes to use.
    #[serde(with = "bigint_ser")]
    pub deal_size: StoragePower,
}

pub type UseBytesParams = BytesParams;
pub type RestoreBytesParams = BytesParams;

#[derive(Clone, Debug, PartialEq, Eq, Serialize_tuple, Deserialize_tuple)]
pub struct RemoveDataCapParams {
    pub verified_client_to_remove: Address,
    #[serde(with = "bigint_ser")]
    pub data_cap_amount_to_remove: DataCap,
    pub verifier_request_1: RemoveDataCapRequest,
    pub verifier_request_2: RemoveDataCapRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize_tuple, Deserialize_tuple)]
pub struct RemoveDataCapRequest {
    pub verifier: Address,
    pub signature: Signature,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize_tuple, Deserialize_tuple)]
pub struct RemoveDataCapReturn {
    pub verified_client: Address,
    #[serde(with = "bigint_ser")]
    pub data_cap_removed: DataCap,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(transparent)]
pub struct RemoveDataCapProposalID(pub u64);

#[derive(Debug, Serialize_tuple, Deserialize_tuple)]
pub struct RemoveDataCapProposal {
    pub verified_client: Address,
    #[serde(with = "bigint_ser")]
    pub data_cap_amount: DataCap,
    pub removal_proposal_id: RemoveDataCapProposalID,
}

pub struct AddrPairKey {
    pub first: Address,
    pub second: Address,
}

impl AddrPairKey {
    pub fn new(first: Address, second: Address) -> Self {
        AddrPairKey {
            first: first,
            second: second,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut first = self.first.to_bytes();
        let mut second = self.second.to_bytes();
        first.append(&mut second);
        first
    }
}
