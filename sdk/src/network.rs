use std::convert::TryInto;

use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::version::NetworkVersion;

pub fn curr_epoch() -> ChainEpoch {
    unsafe { crate::sys::network::curr_epoch() as ChainEpoch }
}

pub fn version() -> NetworkVersion {
    unsafe {
        crate::sys::network::version()
            .try_into()
            .expect("invalid version")
    }
}

pub fn base_fee() -> TokenAmount {
    unsafe {
        let (hi, lo) = crate::sys::network::base_fee();
        TokenAmount::from((hi as u128) << 64 | lo as u128)
    }
}
