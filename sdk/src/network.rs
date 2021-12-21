use std::convert::TryInto;

use crate::sys;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::version::NetworkVersion;

pub fn curr_epoch() -> ChainEpoch {
    unsafe { sys::network::curr_epoch() as ChainEpoch }
}

pub fn version() -> NetworkVersion {
    unsafe { sys::network::version().try_into().expect("invalid version") }
}

pub fn base_fee() -> TokenAmount {
    unsafe {
        let (hi, lo) = sys::network::base_fee();
        TokenAmount::from((hi as u128) << 64 | lo as u128)
    }
}
