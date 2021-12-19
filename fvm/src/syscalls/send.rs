use crate::kernel::BlockId;
use crate::message::Message;
use crate::{
    kernel::{ClassifyResult, Result},
    Kernel,
};
use anyhow::Context as _;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use fvm_shared::encoding::{from_slice, to_vec, DAG_CBOR};
use num_traits::FromPrimitive;
use wasmtime::Caller;

use super::Context;

/// Send a message to another actor. The result is placed as a CBOR-encoded
/// receipt in the block registry, and can be retrieved by the returned BlockId.
///
/// TODO result is a Receipt, but messages within a call stack don't
///  actually produce receipts.
///  See https://github.com/filecoin-project/fvm/issues/168.
///
/// TODO the param should probably not be a Message, but rather a tuple of
///  (to, method (for now), params, value), or a struct encapsulating those.
pub fn send(
    caller: &mut Caller<'_, impl Kernel>,
    msg_off: u32, // Message
    msg_len: u32,
) -> Result<BlockId> {
    let (k, mut mem) = caller.kernel_and_memory()?;
    let msg: Message = mem.read_cbor(msg_off, msg_len)?;
    // An execution error here means that something went wrong in the FVM.
    // Actor errors are communicated in the receipt.
    let receipt = k.send(msg).or_fatal()?;
    let ser = to_vec(&receipt).or_fatal()?;
    let id = k.block_create(DAG_CBOR, ser.as_slice())?;
    Ok(id)
}
