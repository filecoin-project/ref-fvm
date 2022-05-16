use fvm_ipld_encoding::RawBytes;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::{ActorID, MethodNum};

use crate::kernel::SyscallError;

/// Execution Trace, only for informational and debugging purposes.
pub type ExecutionTrace = Vec<ExecutionEvent>;

#[derive(Clone, Debug)]
pub enum ExecutionEvent {
    Call {
        from: ActorID,
        to: Address,
        method: MethodNum,
        params: RawBytes,
        value: TokenAmount,
    },
    CallReturn(RawBytes),
    CallAbort(ExitCode),
    CallError(SyscallError),
}
