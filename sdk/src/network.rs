use std::convert::TryInto;

use fvm_shared::{clock::ChainEpoch, econ::TokenAmount, version::NetworkVersion};

use crate::{
    error::{IntoSyscallResult, SyscallResult},
    sys,
};

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
