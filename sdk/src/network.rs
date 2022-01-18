use std::convert::TryInto;

use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::version::NetworkVersion;

use crate::sys;

pub fn curr_epoch() -> ChainEpoch {
    unsafe {
        sys::network::curr_epoch()
            // infallible
            .expect("failed to get current epoch")
    }
}

pub fn version() -> NetworkVersion {
    unsafe {
        sys::network::version()
            .expect("failed to get network version")
            .try_into()
            .expect("invalid version")
    }
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
