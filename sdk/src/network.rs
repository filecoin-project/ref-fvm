use std::convert::TryInto;

use crate::invocation::TokenAmount;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::version::NetworkVersion;

pub unsafe fn curr_epoch() -> ChainEpoch {
    crate::sys::network::curr_epoch() as ChainEpoch
}

pub unsafe fn version() -> NetworkVersion {
    crate::sys::network::version()
        .try_into()
        .expect("invalid version")
}

pub fn base_fee() -> TokenAmount {
    unsafe {
        let (hi, lo) = crate::sys::network::base_fee(10);
        TokenAmount::from((hi as u128) << 64 | lo as u128)
    }
}
