use fvm_ipld_encoding::RawBytes;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::{ActorID, MethodNum};

use crate::call_manager::InvocationResult;
use crate::kernel::SyscallError;

/// Execution Trace, only for informational and debugging purposes.
pub type ExecutionTrace = Vec<ExecutionEvent>;

#[derive(Clone, Debug)]
pub enum ExecutionEvent {
    Call(SendParams),
    Return(Result<InvocationResult, SyscallError>),
}

#[derive(Clone, Debug)]
pub struct SendParams {
    pub from: ActorID,
    pub to: Address,
    pub method: MethodNum,
    pub params: RawBytes,
    pub value: TokenAmount,
}
