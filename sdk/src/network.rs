use std::convert::TryInto;

use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::version::NetworkVersion;

use crate::{sys, SyscallResult};

pub fn curr_epoch() -> SyscallResult<ChainEpoch> {
    unsafe { Ok(sys::network::curr_epoch()? as ChainEpoch) }
}

pub fn version() -> SyscallResult<NetworkVersion> {
    unsafe {
        Ok(sys::network::version()?
            .try_into()
            .expect("invalid version"))
    }
}

pub fn base_fee() -> SyscallResult<TokenAmount> {
    unsafe {
        let v = sys::network::base_fee()?;
        Ok(v.into())
    }
}

pub fn total_fil_circ_supply() -> SyscallResult<TokenAmount> {
    unsafe {
        let v = sys::network::total_fil_circ_supply()?;
        Ok(v.into())
    }
}
