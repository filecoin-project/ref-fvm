use std::convert::TryInto;

use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::version::NetworkVersion;

use crate::sys;
use crate::vm::INVOCATION_CONTEXT;


/// Panics inside validate context
pub fn curr_epoch() -> ChainEpoch {
    INVOCATION_CONTEXT.network_curr_epoch
}


/// Panics inside validate context
pub fn version() -> NetworkVersion {
    INVOCATION_CONTEXT
        .network_version
        .try_into()
        .expect("invalid network version")
}

/// Panics inside validate context
pub fn base_fee() -> TokenAmount {
    unsafe {
        sys::network::base_fee()
            .expect("failed to get base fee")
            .into()
    }
}

/// Panics inside validate context
pub fn total_fil_circ_supply() -> TokenAmount {
    unsafe {
        sys::network::total_fil_circ_supply()
            .expect("failed to get circulating supply")
            .into()
    }
}
