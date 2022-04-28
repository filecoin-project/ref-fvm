use std::convert::TryInto;

use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::version::NetworkVersion;

use crate::sys;
use crate::vm::MESSAGE_DETAILS;

pub fn curr_epoch() -> ChainEpoch {
    MESSAGE_DETAILS.network_curr_epoch
}

pub fn version() -> NetworkVersion {
    MESSAGE_DETAILS
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
