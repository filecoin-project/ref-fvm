use std::convert::TryInto;

use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::{ErrorNumber, ExitCode};
use fvm_shared::MethodNum;

use crate::{sys, SyscallResult, NO_DATA_BLOCK_ID};

/// The outcome of a `Send`, covering its ExitCode and optional return data
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Response {
    pub exit_code: ExitCode,
    pub return_data: Option<IpldBlock>,
}

/// Sends a message to another actor.
pub fn send(
    to: &Address,
    method: MethodNum,
    params: Option<IpldBlock>,
    value: TokenAmount,
) -> SyscallResult<Response> {
    let recipient = to.to_bytes();
    let value: sys::TokenAmount = value
        .try_into()
        .map_err(|_| ErrorNumber::InsufficientFunds)?;
    unsafe {
        // Insert parameters as a block. Nil parameters is represented as the
        // NO_DATA_BLOCK_ID block ID in the FFI interface.
        let params_id = match params {
            Some(p) => sys::ipld::block_create(p.codec, p.data.as_ptr(), p.data.len() as u32)?,
            None => NO_DATA_BLOCK_ID,
        };

        // Perform the syscall to send the message.
        let fvm_shared::sys::out::send::Send {
            exit_code,
            return_id,
            return_codec,
            return_size,
        } = sys::send::send(
            recipient.as_ptr(),
            recipient.len() as u32,
            method,
            params_id,
            value.hi,
            value.lo,
        )?;

        // Process the result.
        let exit_code = ExitCode::new(exit_code);
        let return_data = match exit_code {
            ExitCode::OK if return_id != NO_DATA_BLOCK_ID => {
                // Allocate a buffer to read the return data.
                let mut bytes = vec![0; return_size as usize];

                // Now read the return data.
                let unread = sys::ipld::block_read(return_id, 0, bytes.as_mut_ptr(), return_size)?;
                assert_eq!(0, unread);
                Some(IpldBlock {
                    codec: return_codec,
                    data: bytes.to_vec(),
                })
            }
            _ => None,
        };

        Ok(Response {
            exit_code,
            return_data,
        })
    }
}
