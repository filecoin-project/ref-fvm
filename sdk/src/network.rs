// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::Cid;
use fvm_shared::chainid::ChainID;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ErrorNumber;
use fvm_shared::sys::out::network::NetworkContext;
use fvm_shared::version::NetworkVersion;
use fvm_shared::MAX_CID_LEN;

use crate::error::EpochBoundsError;
use crate::sys;

lazy_static::lazy_static! {
    pub(crate) static ref NETWORK_CONTEXT: NetworkContext = {
        unsafe {
            sys::network::context().expect("failed to lookup network context")
        }
    };
}

pub fn chain_id() -> ChainID {
    NETWORK_CONTEXT.chain_id.into()
}

pub fn curr_epoch() -> ChainEpoch {
    NETWORK_CONTEXT.epoch
}

pub fn version() -> NetworkVersion {
    NETWORK_CONTEXT.network_version
}

pub fn base_fee() -> TokenAmount {
    NETWORK_CONTEXT.base_fee.into()
}

pub fn total_fil_circ_supply() -> TokenAmount {
    unsafe {
        sys::network::total_fil_circ_supply()
            .expect("failed to get circulating supply")
            .into()
    }
}

/// Returns the current block time in seconds since the EPOCH.
pub fn tipset_timestamp() -> u64 {
    NETWORK_CONTEXT.timestamp
}

/// Returns the tipset CID of the specified epoch, if available. Allows querying from now up to
/// finality (900 epochs).
pub fn tipset_cid(epoch: ChainEpoch) -> Result<Cid, EpochBoundsError> {
    let mut buf = [0u8; MAX_CID_LEN];

    unsafe {
        match sys::network::tipset_cid(epoch, buf.as_mut_ptr(), MAX_CID_LEN as u32) {
            Ok(len) => Ok(Cid::read_bytes(&buf[..len as usize]).expect("invalid cid")),
            Err(ErrorNumber::IllegalArgument) => Err(EpochBoundsError::Invalid),
            Err(ErrorNumber::LimitExceeded) => Err(EpochBoundsError::ExceedsLookback),
            Err(other) => panic!("unexpected cid resolution failure: {}", other),
        }
    }
}
