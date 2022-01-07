use fvm_shared::error::ExitCode;

use wasmtime::{Caller, Linker, Trap, WasmRet, WasmTy};

use crate::kernel::{ExecutionError, SyscallError};
use crate::Kernel;

use super::error::trap_from_error;
use super::{Context, Memory};

// TODO: we should consider implementing a proc macro attribute for syscall functions instead of
// this type nonsense. But this was faster and will "work" for now.

/// Binds syscalls to a linker, converting the returned error according to the syscall convention:
///
/// 1. If the error is a syscall error, it's returned as the first return value.
/// 2. If the error is a fatal error, a Trap is returned.
pub(super) trait BindSyscall<Args, Ret, Func> {
    /// Bind a syscall to the linker.
    ///
    /// The return type will be automatically adjusted to return `Result<(u32, ...), Trap>` where
    /// `u32` is the error code and `...` is the previous return type. For example:
    ///
    /// - `kernel::Result<()>` will become `kernel::Result<u32>`.
    /// - `kernel::Result<i64>` will become `Result<(u32, i64), Trap>`.
    /// - `kernel::Result<(i32, i32)>` will become `Result<(u32, i32, i32), Trap>`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// mod my_module {
    ///     pub fn zero(kernel: &mut impl Kernel, memory: &mut [u8], arg: i32) -> crate::fvm::kernel::Result<i32> {
    ///         Ok(0)
    ///     }
    /// }
    /// let engine = wasmtime::Engine::default();
    /// let mut linker = wasmtime::Linker::new(&engine);
    /// linker.bind("my_module", "zero", my_module::zero);
    /// ```
    fn bind(&mut self, module: &str, name: &str, syscall: Func) -> anyhow::Result<&mut Self> {
        self.bind_syscall(module, name, syscall, false)
    }

    /// Bind a syscall to the linker, but preserve last syscall error.
    fn bind_keep_error(
        &mut self,
        module: &str,
        name: &str,
        syscall: Func,
    ) -> anyhow::Result<&mut Self> {
        self.bind_syscall(module, name, syscall, true)
    }

    /// Bind a syscall to the linker.
    fn bind_syscall(
        &mut self,
        module: &str,
        name: &str,
        syscall: Func,
        keep_error: bool,
    ) -> anyhow::Result<&mut Self>;
}

/// The helper trait used by `BindSyscall` to convert kernel results with execution errors into
/// results that can be handled by wasmtime. See the documentation on `BindSyscall` for details.
#[doc(hidden)]
pub trait IntoSyscallResult: Sized {
    type Value;
    fn into<K: Kernel>(self, k: &mut K) -> Result<Self::Value, Trap>;
}

// Implementation for syscalls that want to trap directly.
impl<T> IntoSyscallResult for Result<T, Trap> {
    type Value = T;
    fn into<K: Kernel>(self, _k: &mut K) -> Result<Self::Value, Trap> {
        self
    }
}

// Unfortunately, we can't implement this for _all_ functions. So we implement it for functions of up to 6 arguments.
macro_rules! impl_bind_syscalls {
    ($($t:ident)*) => {
        #[allow(non_snake_case)]
        impl<$($t,)* Ret, K, Func> BindSyscall<($($t,)*), Ret, Func> for Linker<K>
        where
            K: Kernel,
            Func: Fn(Context<'_, K> $(, $t)*) -> Ret + Send + Sync + 'static,
            Ret: IntoSyscallResult,
            Ret::Value: WasmRet,
           $($t: WasmTy,)*
        {
            fn bind_syscall(&mut self, module: &str, name: &str, syscall: Func, keep_error: bool) -> anyhow::Result<&mut Self> {
                self.func_wrap(module, name, move |mut caller: Caller<'_, K> $(, $t: $t)*| {
                    let (memory, kernel) = caller
                        .get_export("memory")
                        .and_then(|m| m.into_memory())
                        .ok_or_else(|| Trap::new("failed to lookup actor memory"))?
                        .data_and_store_mut(&mut caller);

                    if !keep_error {
                        kernel.clear_error();
                    }
                    syscall(Context{ kernel, memory: Memory::new(memory) } $(, $t)*).into(kernel)
                })
            }
        }

        #[allow(non_snake_case)]
        impl< $($t),* > IntoSyscallResult for crate::kernel::Result<($($t,)*)>
        where
            $($t: Default + WasmTy,)*
        {
            type Value = (u32, $($t),*);
            fn into<K: Kernel>(self, k: &mut K) -> Result<Self::Value, Trap> {
                use ExecutionError::*;
                match self {
                    Ok(($($t,)*)) => Ok((ExitCode::Ok as u32, $($t),*)),
                    Err(Syscall(err @ SyscallError(_, code))) if err.is_recoverable() => {
                        k.push_syscall_error(err);
                        Ok((code as u32, $($t::default()),*))
                    },
                    Err(err) => Err(trap_from_error(err)),
                }
            }
        }
    }
}

impl_bind_syscalls!();
impl_bind_syscalls!(A);
impl_bind_syscalls!(A B);
impl_bind_syscalls!(A B C);
impl_bind_syscalls!(A B C D);
impl_bind_syscalls!(A B C D E);
impl_bind_syscalls!(A B C D E F);

// We've now implemented it for all "tuple" return types, but we still need to implement it for
// returning a single non-tuple type. Unfortunately, we can't do this generically without
// conflicting with the tuple impls, so we implement it explicitly for all value types we support.
macro_rules! impl_bind_syscalls_single_return {
    ($($t:ty)*) => {
        $(
            impl IntoSyscallResult for crate::kernel::Result<$t> {
                type Value = (u32, $t);
                fn into<K: Kernel>(self, k: &mut K) -> Result<Self::Value, Trap> {
                    use ExecutionError::*;
                    match self {
                        Ok(v) => Ok((ExitCode::Ok as u32, v)),
                        Err(Syscall(err @ SyscallError(_, code))) if err.is_recoverable() => {
                            k.push_syscall_error(err);
                            Ok((code as u32, Default::default()))
                        }
                        Err(err) => Err(trap_from_error(err)),
                    }
                }
            }
        )*
    };
}
impl_bind_syscalls_single_return!(u32 i32 u64 i64 f32 f64);
