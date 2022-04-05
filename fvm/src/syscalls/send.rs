use fvm_ipld_encoding::DAG_CBOR;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::sys;

use super::Context;
use crate::call_manager::{InvocationResult, NO_DATA_BLOCK_ID};
use crate::kernel::Result;
use crate::Kernel;

/// Send a message to another actor. The result is placed as a CBOR-encoded
/// receipt in the block registry, and can be retrieved by the returned BlockId.
pub fn send(
    context: Context<'_, impl Kernel>,
    recipient_off: u32,
    recipient_len: u32,
    method: u64,
    params_id: u32,
    value_hi: u64,
    value_lo: u64,
) -> Result<sys::out::send::Send> {
    let recipient: Address = context.memory.read_address(recipient_off, recipient_len)?;
    let value = TokenAmount::from((value_hi as u128) << 64 | value_lo as u128);
    // TODO: consider just passing the block ID directly into the kernel.
    let (code, params) = if params_id > NO_DATA_BLOCK_ID {
        context.kernel.block_get(params_id)?
    } else {
        (DAG_CBOR, Vec::new())
    };
    debug_assert_eq!(code, DAG_CBOR);
    // An execution error here means that something went wrong in the FVM.
    // Actor errors are communicated in the receipt.
    let (exit_code, return_id) =
        match context
            .kernel
            .send(&recipient, method, &params.into(), &value)?
        {
            InvocationResult::Return(value) => (
                ExitCode::OK.value(),
                context.kernel.block_create(DAG_CBOR, value.bytes())?,
            ),
            InvocationResult::Failure(code) => (code.value(), 0),
        };
    Ok(sys::out::send::Send {
        exit_code,
        return_id,
    })
}
