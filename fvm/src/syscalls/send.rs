use anyhow::Context as _;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ErrorNumber::GasLimitExceeded;
use fvm_shared::sys::{self, SendFlags};

use super::Context;
use crate::gas::Gas;
use crate::kernel::{ClassifyResult, ExecutionError, Result, SendResult, SyscallError};
use crate::Kernel;

/// Send a message to another actor. The result is placed as a CBOR-encoded
/// receipt in the block registry, and can be retrieved by the returned BlockId.
#[allow(clippy::too_many_arguments)]
pub fn send(
    context: Context<'_, impl Kernel>,
    recipient_off: u32,
    recipient_len: u32,
    method: u64,
    params_id: u32,
    value_hi: u64,
    value_lo: u64,
    gas_limit: u64,
    flags: u64,
) -> Result<sys::out::send::Send> {
    let recipient: Address = context.memory.read_address(recipient_off, recipient_len)?;
    let value = TokenAmount::from_atto((value_hi as u128) << 64 | value_lo as u128);

    // Only pass on the gas limit, and subsequently lower errors, if the caller requested a gas
    // limit below their gas available.
    let effective_gas_limit = (gas_limit > 0)
        .then(|| Gas::new(gas_limit as i64))
        .filter(|gas_limit| gas_limit < &context.kernel.gas_available());

    let flags = SendFlags::from_bits(flags)
        .with_context(|| format!("invalid send flags: {flags}"))
        .or_illegal_argument()?;

    // An execution error here means that something went wrong in the FVM.
    // Actor errors are communicated in the receipt.
    let mut res = context.kernel.send(
        &recipient,
        method,
        params_id,
        &value,
        effective_gas_limit,
        flags,
    );

    // Lower the out of gas to a syscall error.
    if matches!(res, Err(ExecutionError::OutOfGas) if effective_gas_limit.is_some()) {
        res = Err(SyscallError::new(GasLimitExceeded, "nested call out of gas").into())
    }

    let SendResult {
        block_id,
        block_stat,
        exit_code,
    } = res?;

    Ok(sys::out::send::Send {
        exit_code: exit_code.value(),
        return_id: block_id,
        return_codec: block_stat.codec,
        return_size: block_stat.size,
    })
}
