use crate::message::NO_DATA_BLOCK_ID;
use crate::sys;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
// no_std
use crate::error::{IntoSyscallResult, SyscallResult};
use fvm_shared::encoding::{RawBytes, DAG_CBOR};
use fvm_shared::error::ExitCode::{self, ErrIllegalArgument};
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
    let mut value_iter = value.iter_u64_digits();
    let value_lo = value_iter.next().unwrap();
    let value_hi = value_iter.next().unwrap_or(0);
    if value_iter.next().is_some() {
        return Err(ErrIllegalArgument);
    };
    unsafe {
        // Send the message.
        let params_id = if params.len() == 0 {
            NO_DATA_BLOCK_ID
        } else {
            sys::ipld::create(DAG_CBOR, params.as_ptr(), params.len() as u32)
                .into_syscall_result()?
        };
        let (exit_code, return_id) = sys::send::send(
            recipient.as_ptr(),
            recipient.len() as u32,
            method,
            params_id,
            value_hi,
            value_lo,
        )
        .into_syscall_result()?;
        if exit_code != ExitCode::Ok as u32 {
            return Ok(Receipt {
                exit_code: ExitCode::from_u32(exit_code).unwrap_or(ExitCode::ErrIllegalState),
                return_data: Default::default(),
                gas_used: 0,
            });
        }
        let return_data = if return_id == NO_DATA_BLOCK_ID {
            RawBytes::default()
        } else {
            // Allocate a buffer to read the result.
            let (_, length) = sys::ipld::stat(return_id).into_syscall_result()?;
            let mut bytes = Vec::with_capacity(length as usize);
            // Now read the result.
            let read =
                sys::ipld::read(return_id, 0, bytes.as_mut_ptr(), length).into_syscall_result()?;
            assert_eq!(read, length);
            RawBytes::from(bytes)
        };
        // Deserialize the receipt.
        Ok(Receipt {
            exit_code: ExitCode::Ok,
            return_data,
            gas_used: 0,
        })
    }
}
