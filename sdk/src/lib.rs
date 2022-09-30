pub mod actor;
pub mod crypto;
pub mod debug;
pub mod error;
pub mod gas;
pub mod ipld;
pub mod message;
pub mod network;
pub mod rand;
pub mod send;
pub mod sself;
pub mod sys;
pub mod vm;

#[cfg(feature = "testing")]
pub mod testing;

/// The maximum actor address length (class 2 addresses).
pub const MAX_ACTOR_ADDR_LEN: usize = 21;

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
