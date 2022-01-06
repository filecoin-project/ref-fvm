use fvm_shared::error::ExitCode;
use num_traits::FromPrimitive;

pub mod actor;
pub mod crypto;
#[cfg(feature = "debug")]
pub mod debug;
pub mod gas;
pub mod ipld;
pub mod message;
pub mod network;
pub mod rand;
pub mod send;
pub mod sself;
pub mod validation;
pub mod vm;

use super::SyscallResult;

macro_rules! impl_from_syscall_result {
    ($name:ident($($t:ident),*)) => {

        #[repr(C)]
        #[repr(packed)]
        pub struct $name<$($t),*>(u32 $(,$t)*);

        #[allow(unused_parens, non_snake_case)]
        impl<$($t),*> $name<$($t),*> {
            /// Convert into a normalized SyscallResult
            pub fn into_result(self) -> SyscallResult<($($t),*)> {
                let $name(code $(, $t)*) = self;
                match FromPrimitive::from_u32(code) {
                    Some(ExitCode::Ok) => Ok(($($t),*)),
                    Some(code) if code.is_system_error() => Err(code),
                    Some(code) => panic!("syscall returned non-system error {}", code),
                    None => panic!("syscall returned unrecognized exit code"),
                }
            }
        }
    }
}

impl_from_syscall_result!(SyscallResult0());
impl_from_syscall_result!(SyscallResult1(A));
impl_from_syscall_result!(SyscallResult2(A, B));
impl_from_syscall_result!(SyscallResult3(A, B, C));
impl_from_syscall_result!(SyscallResult4(A, B, C, D));
impl_from_syscall_result!(SyscallResult5(A, B, C, D, E));
