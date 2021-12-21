use fvm_shared::error::ExitCode;
use num_traits::FromPrimitive;

/// SDK functions performing a syscall return a SyscallResult type, where the
/// Error type is an ExitCode. ExitCode::Ok is translated to an Ok result, while
/// error codes are propagated as Err(ExitCode).
///
/// Error messages don't make it across the boundary, but are logged at the FVM
/// level for debugging and informational purposes.
pub type SyscallResult<T> = core::result::Result<T, ExitCode>;

/// When called on a syscall result (either a tuple starting with a u32 or a single u32), this trait
/// converts said result into a SyscallResult, interpreting the leading u32 as an exit code and the
/// remaining values ad the return value.
pub(crate) trait IntoSyscallResult {
    type Value;
    fn into_syscall_result(self) -> SyscallResult<Self::Value>;
}

// Zero results.
impl IntoSyscallResult for u32 {
    type Value = ();
    fn into_syscall_result(self) -> SyscallResult<Self::Value> {
        match FromPrimitive::from_u32(self).expect("syscall returned unrecognized exit code") {
            ExitCode::Ok => Ok(()),
            other => Err(other),
        }
    }
}

// Single result.
impl<T> IntoSyscallResult for (u32, T) {
    type Value = T;
    fn into_syscall_result(self) -> SyscallResult<Self::Value> {
        let (code, val) = self;
        match FromPrimitive::from_u32(code).expect("syscall returned unrecognized exit code") {
            ExitCode::Ok => Ok(val),
            other => Err(other),
        }
    }
}

// Multiple results.
macro_rules! impl_into_syscall_result {
    ($($t:ident)+) => {
        #[allow(non_snake_case)]
        impl<$($t),+> IntoSyscallResult for (u32 $(, $t)+) {
            type Value = ($($t),+);
            fn into_syscall_result(self) -> SyscallResult<Self::Value> {
                let (code $(, $t)+) = self;
                match FromPrimitive::from_u32(code).expect("syscall returned unrecognized exit code") {
                    ExitCode::Ok => Ok(($($t),+)),
                    other => Err(other),
                }
            }
        }
    }
}

impl_into_syscall_result!(A B);
impl_into_syscall_result!(A B C);
impl_into_syscall_result!(A B C D);
impl_into_syscall_result!(A B C D E);
impl_into_syscall_result!(A B C D E F);
