use std::convert::TryFrom;

use fvm_shared::error::ExitCode;
use num_traits::FromPrimitive;

pub mod actor;
pub mod crypto;
#[cfg(feature = "debug")]
pub mod debug;
pub mod fvm;
pub mod gas;
pub mod ipld;
pub mod message;
pub mod network;
pub mod rand;
pub mod send;
pub mod sself;
pub mod validation;

#[repr(transparent)]
pub struct SyscallStatus(u32);

impl TryFrom<SyscallStatus> for ExitCode {
    type Error = u32;
    fn try_from(e: SyscallStatus) -> Result<ExitCode, u32> {
        FromPrimitive::from_u32(e.0).ok_or(e.0)
    }
}
