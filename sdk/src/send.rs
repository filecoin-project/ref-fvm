use std::convert::TryInto;

use crate::{message::NO_DATA_BLOCK_ID, sys, SyscallResult};
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::{RawBytes, DAG_CBOR};
use fvm_shared::error::ExitCode;
use fvm_shared::receipt::Receipt;
use fvm_shared::MethodNum;
use num_traits::FromPrimitive;

/// Sends a message to another actor.
// TODO: Drop the use of receipts here as we don't return the gas used. Alternatively, we _could_
// return gas used?
pub fn send(
    to: &Address,
    method: MethodNum,
    params: RawBytes,
    value: TokenAmount,
) -> SyscallResult<Receipt> {
    let recipient = to.to_bytes();
    let value: fvm_shared::sys::TokenAmount = value
        .try_into()
        .map_err(|_| ExitCode::ErrInsufficientFunds)?;
    unsafe {
        // Insert parameters as a block. Nil parameters is represented as the
        // NO_DATA_BLOCK_ID block ID in the FFI interface.
        let params_id = if params.len() > 0 {
            sys::ipld::create(DAG_CBOR, params.as_ptr(), params.len() as u32)?
        } else {
            NO_DATA_BLOCK_ID
        };

        // Perform the syscall to send the message.
        let fvm_shared::sys::out::send::Send {
            exit_code,
            return_id,
        } = sys::send::send(
            recipient.as_ptr(),
            recipient.len() as u32,
            method,
            params_id,
            value.hi,
            value.lo,
        )?;

        // Process the result.
        let exit_code = ExitCode::from_u32(exit_code).unwrap_or(ExitCode::ErrIllegalState);
        let return_data = match exit_code {
            ExitCode::Ok if return_id != NO_DATA_BLOCK_ID => {
                // Allocate a buffer to read the return data.
                let fvm_shared::sys::out::ipld::IpldStat { size, .. } = sys::ipld::stat(return_id)?;
                let mut bytes = Vec::with_capacity(size as usize);

                // Now read the return data.
                let read = sys::ipld::read(return_id, 0, bytes.as_mut_ptr(), size)?;
                assert_eq!(read, size);
                RawBytes::from(bytes)
            }
            _ => Default::default(),
        };

        Ok(Receipt {
            exit_code,
            return_data,
            gas_used: 0,
        })
    }
}
