use crate::call_manager::InvocationResult;
use crate::kernel::BlockId;
use crate::{kernel::Result, Kernel};
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::DAG_CBOR;
use fvm_shared::error::ExitCode;

use super::Memory;

/// Send a message to another actor. The result is placed as a CBOR-encoded
/// receipt in the block registry, and can be retrieved by the returned BlockId.
///
/// TODO result is a Receipt, but messages within a call stack don't
///  actually produce receipts.
///  See https://github.com/filecoin-project/fvm/issues/168.
pub fn send(
    kernel: &mut impl Kernel,
    memory: &mut [u8],
    recipient_off: u32,
    recipient_len: u32,
    method: u64,
    params_id: u32,
    value_hi: u64,
    value_lo: u64,
) -> Result<(u32, BlockId)> {
    let recipient: Address = memory.read_address(recipient_off, recipient_len)?;
    let value = TokenAmount::from((value_hi as u128) << 64 | value_lo as u128);
    let (code, params) = kernel.block_get(params_id)?;
    debug_assert_eq!(code, DAG_CBOR);
    // An execution error here means that something went wrong in the FVM.
    // Actor errors are communicated in the receipt.
    Ok(
        match kernel.send(&recipient, method, &params.into(), &value)? {
            InvocationResult::Return(value) => (
                ExitCode::Ok as u32,
                kernel.block_create(DAG_CBOR, value.bytes())?,
            ),
            InvocationResult::Failure(code) => (code as u32, 0),
        },
    )
}
