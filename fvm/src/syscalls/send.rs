use crate::kernel::BlockId;
use crate::{
    kernel::{ClassifyResult, Result},
    Kernel,
};
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{to_vec, DAG_CBOR};
use wasmtime::Caller;

use super::Context;

/// Send a message to another actor. The result is placed as a CBOR-encoded
/// receipt in the block registry, and can be retrieved by the returned BlockId.
///
/// TODO result is a Receipt, but messages within a call stack don't
///  actually produce receipts.
///  See https://github.com/filecoin-project/fvm/issues/168.
pub fn send(
    caller: &mut Caller<'_, impl Kernel>,
    recipient_off: u32,
    recipient_len: u32,
    method: u64,
    params_id: u32,
    value_hi: u64,
    value_lo: u64,
) -> Result<BlockId> {
    let (k, memory) = caller.kernel_and_memory()?;
    let recipient: Address = memory.read_address(recipient_off, recipient_len)?;
    let value = TokenAmount::from((value_hi as u128) << 64 | value_lo as u128);
    let (code, params) = k.block_get(params_id)?;
    debug_assert_eq!(code, DAG_CBOR);
    // An execution error here means that something went wrong in the FVM.
    // Actor errors are communicated in the receipt.
    let receipt = k.send(&recipient, method, &params.into(), &value)?;
    let ser = to_vec(&receipt).or_fatal()?;
    let id = k.block_create(DAG_CBOR, ser.as_slice())?;
    Ok(id)
}
