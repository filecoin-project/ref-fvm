use std::convert::TryInto;

use crate::error::{IntoSyscallResult, SyscallResult};
use crate::sys;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::version::NetworkVersion;

pub fn curr_epoch() -> SyscallResult<ChainEpoch> {
    unsafe { Ok(sys::network::curr_epoch().into_syscall_result()? as ChainEpoch) }
}

pub fn version() -> SyscallResult<NetworkVersion> {
    unsafe {
        Ok(sys::network::version()
            .into_syscall_result()?
            .try_into()
            .expect("invalid version"))
    }
}

pub fn base_fee() -> SyscallResult<TokenAmount> {
    unsafe {
        let (hi, lo) = sys::network::base_fee().into_syscall_result()?;
        Ok(TokenAmount::from((hi as u128) << 64 | lo as u128))
    }
}

pub fn total_fil_circ_supply() -> SyscallResult<TokenAmount> {
    unsafe {
        let (hi, lo) = sys::network::total_fil_circ_supply().into_syscall_result()?;
        Ok(TokenAmount::from((hi as u128) << 64 | lo as u128))
    }
}
