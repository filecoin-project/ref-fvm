use std::convert::TryInto;

use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::version::NetworkVersion;
use cid::Cid;
use fvm_shared::error::ErrorNumber;
use fvm_shared::MAX_CID_LEN;

use crate::sys;
use crate::vm::INVOCATION_CONTEXT;

pub fn curr_epoch() -> ChainEpoch {
    INVOCATION_CONTEXT.network_curr_epoch
}

pub fn version() -> NetworkVersion {
    INVOCATION_CONTEXT
        .network_version
        .try_into()
        .expect("invalid network version")
}

pub fn base_fee() -> TokenAmount {
    unsafe {
        sys::network::base_fee()
            .expect("failed to get base fee")
            .into()
    }
}

pub fn total_fil_circ_supply() -> TokenAmount {
    unsafe {
        sys::network::total_fil_circ_supply()
            .expect("failed to get circulating supply")
            .into()
    }
}

pub fn tipset_timestamp() -> u64 {
    unsafe { sys::network::tipset_timestamp() }.expect("failed to get timestamp")
}

pub fn tipset_cid(epoch: i64) -> Option<Cid> {
    let mut buf = [0u8; MAX_CID_LEN];

    unsafe {
        match sys::network::tipset_cid(epoch, buf.as_mut_ptr(), MAX_CID_LEN as u32) {
            Ok(len) => Some(Cid::read_bytes(&buf[..len as usize]).expect("invalid cid")),
            Err(ErrorNumber::NotFound) => None,
            Err(other) => panic!("unexpected cid resolution failure: {}", other),
        }
    }
}
