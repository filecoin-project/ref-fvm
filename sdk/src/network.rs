use std::convert::TryInto;

use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::version::NetworkVersion;

use crate::message::MESSAGE_DETAILS;

pub fn curr_epoch() -> ChainEpoch {
    MESSAGE_DETAILS.curr_epoch
}

pub fn version() -> NetworkVersion {
    MESSAGE_DETAILS
        .version
        .try_into()
        .expect("invalid network version")
}

pub fn base_fee() -> TokenAmount {
    MESSAGE_DETAILS.base_fee.try_into().expect("invalid bigint")
}

pub fn total_fil_circ_supply() -> TokenAmount {
    MESSAGE_DETAILS
        .circulating_supply
        .try_into()
        .expect("invalid bigint")
}
