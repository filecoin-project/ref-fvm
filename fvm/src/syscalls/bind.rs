use std::mem;

use fvm_shared::error::ErrorNumber;
use wasmtime::{Caller, Linker, Trap, WasmTy};

use super::error::trap_from_error;
use super::Context;
use crate::kernel::{self, ExecutionError, Kernel, SyscallError};

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
    type Value: Copy + Sized + 'static;
    fn into(self) -> Result<Result<Self::Value, SyscallError>, Trap>;
}
// Implementation for syscalls that want to trap directly.
impl<T> IntoSyscallResult for Result<T, Trap>
where
    T: Copy + Sized + 'static,
{
    type Value = T;
    fn into(self) -> Result<Result<Self::Value, SyscallError>, Trap> {
        self.map(Ok)
    }
}

impl<T> IntoSyscallResult for kernel::Result<T>
where
    T: Copy + Sized + 'static,
{
    type Value = T;
    fn into(self) -> Result<Result<Self::Value, SyscallError>, Trap> {
        match self {
            Ok(value) => Ok(Ok(value)),
            Err(ExecutionError::Syscall(err)) => Ok(Err(err)),
            Err(e) => Err(trap_from_error(e)),
        }
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
           $($t: WasmTy,)*
        {
            fn bind_syscall(&mut self, module: &str, name: &str, syscall: Func, keep_error: bool) -> anyhow::Result<&mut Self> {
                if mem::size_of::<Ret::Value>() == 0 {
                    // If we're returning a zero-sized "value", we return no value therefore and expect no out pointer.
                    self.func_wrap(module, name, move |mut caller: Caller<'_, K> $(, $t: $t)*| {
                        let mut ctx = Context::try_from(&mut caller)?;
                        if !keep_error {
                            ctx.kernel.clear_error();
                        }

                        Ok(match syscall(ctx.reborrow() $(, $t)*).into()? {
                            Ok(_) => 0,
                            Err(err) => {
                                let code = err.1;
                                ctx.kernel.push_syscall_error(err);
                                code as u32
                            },
                        })
                    })
                } else {
                    // If we're returning an actual value, we need to write it back into the wasm module's memory.
                    self.func_wrap(module, name, move |mut caller: Caller<'_, K>, ret: u32 $(, $t: $t)*| {
                        let mut ctx = Context::try_from(&mut caller)?;

                        if !keep_error {
                            ctx.kernel.clear_error();
                        }

                        // We need to check to make sure we can store the return value _before_ we do anything.
                        if (ret as u64) > (ctx.memory.len() as u64)
                            || ctx.memory.len() - (ret as usize) < mem::size_of::<Ret::Value>() {
                            let code = ErrorNumber::IllegalArgument;
                            ctx.kernel.push_syscall_error(SyscallError(format!("no space for return value"), code));
                            return Ok(code as u32);
                        }

                        Ok(match syscall(ctx.reborrow() $(, $t)*).into()? {
                            Ok(value) => {
                                unsafe { *(ctx.memory.as_mut_ptr().offset(ret as isize) as *mut Ret::Value) = value };
                                0
                            },
                            Err(err) => {
                                let code = err.1;
                                ctx.kernel.push_syscall_error(err);
                                code as u32
                            },
                        })
                    })
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
