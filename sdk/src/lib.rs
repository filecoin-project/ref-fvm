// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
pub mod actor;
pub mod crypto;
pub mod debug;
pub mod error;
pub mod event;
pub mod gas;
pub mod ipld;
pub mod message;
pub mod network;
pub mod rand;
pub mod send;
pub mod sself;
pub mod sys;
pub mod vm;

/// BlockID representing nil parameters or return data.
pub const NO_DATA_BLOCK_ID: u32 = 0;

// TODO: provide a custom panic handler?

#[inline]
pub(crate) fn status_code_to_bool(code: i32) -> bool {
    code == 0
}

/// SDK functions performing a syscall return a SyscallResult type, where the
/// Error type is an ExitCode. ExitCode::Ok is translated to an Ok result, while
/// error codes are propagated as Err(ExitCode).
///
/// Error messages don't make it across the boundary, but are logged at the FVM
/// level for debugging and informational purposes.
pub type SyscallResult<T> = core::result::Result<T, fvm_shared::error::ErrorNumber>;

/// Initialize the FVM SDK. Calling this function optional but encouraged.
///
/// At the moment, this will:
///
/// 1. Initialize logging (if "debug mode" is enabled).
/// 2. Setup a panic handler for easier debugging.
///
/// In the future, this may perform additional setup operations, but will never incure more than a
/// minimal runtime cost.
pub fn initialize() {
    debug::init_logging();
    vm::set_panic_handler();
}

fn build_response(send: fvm_shared::sys::out::send::Send) -> SyscallResult<fvm_shared::Response> {
    let exit_code = fvm_shared::error::ExitCode::new(send.exit_code);
    let return_data = if send.return_id == NO_DATA_BLOCK_ID {
        None
    } else {
        // Allocate a buffer to read the return data.
        let mut bytes = vec![0; send.return_size as usize];

        unsafe {
            // Now read the return data.
            let unread =
                sys::ipld::block_read(send.return_id, 0, bytes.as_mut_ptr(), send.return_size)?;
            assert_eq!(0, unread);
        }

        Some(fvm_ipld_encoding::ipld_block::IpldBlock {
            codec: send.return_codec,
            data: bytes.to_vec(),
        })
    };

    Ok(fvm_shared::Response {
        exit_code,
        return_data,
    })
}
