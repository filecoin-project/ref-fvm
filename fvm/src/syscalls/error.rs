use std::sync::Mutex;

use anyhow::Context;
use derive_more::Display;
use fvm_shared::error::ExitCode;
use num_traits::FromPrimitive;
use wasmtime::{Caller, Linker, Trap, WasmRet, WasmTy};

use crate::kernel::{ClassifyResult, ExecutionError, SyscallError};
use crate::receipt::Receipt;
use crate::Kernel;

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
    /// ```rust
    /// mod my_module {
    ///     fn zero(caller: wasmtime::Caller<'a, ()>, arg: i32) -> crate::kernel::Result<i32> {
    ///         0
    ///     }
    /// }
    /// let engine = wasmtime::Engine::default();
    /// let mut linker = wasmtime::Linker::new(&engine);
    /// linker.bind("my_module", "zero", my_module::zero);
    /// ```
    fn bind(&mut self, module: &str, name: &str, syscall: Func) -> anyhow::Result<&mut Self>;
}

/// The helper trait used by `BindSyscall` to convert kernel results with execution errors into
/// results that can be handled by wasmtime. See the documentation on `BindSyscall` for details.
#[doc(hidden)]
pub trait IntoSyscallResult: Sized {
    type Value;
    fn into<K: Kernel>(self, k: &mut K) -> Result<Self::Value, Trap>;
}

// Unfortunately, we can't implement this for _all_ functions. So we implement it for functions of up to 6 arguments.
macro_rules! impl_bind_syscalls {
    ($($t:ident)*) => {
        #[allow(non_snake_case)]
        impl<$($t,)* Ret, K, Func> BindSyscall<($($t,)*), Ret, Func> for Linker<K>
        where
            K: Kernel,
            Func: for<'a> Fn(&'a mut Caller<'_, K> $(, $t)*) -> Ret + Send + Sync + 'static,
            Ret: IntoSyscallResult,
            Ret::Value: WasmRet,
           $($t: WasmTy,)*
        {
            fn bind(&mut self, module: &str, name: &str, syscall: Func) -> anyhow::Result<&mut Self> {
                self.func_wrap(module, name, move |mut caller: Caller<'_, K> $(, $t: $t)*| {
                    syscall(&mut caller $(, $t)*).into(caller.data_mut())
                })
            }
        }

        #[allow(non_snake_case)]
        impl< $($t),* > IntoSyscallResult for crate::kernel::Result<($($t,)*)>
        where
            $($t: Default + WasmTy,)*
        {
            type Value = (u32, $($t),*);
            fn into<K: Kernel>(self, _k: &mut K) -> Result<Self::Value, Trap> {
                // TODO: log the message here with the kernel.
                match self {
                    Ok(($($t,)*)) => Ok((ExitCode::Ok as u32, $($t),*)),
                    Err(ExecutionError::Syscall(SyscallError(_msg, code))) => Ok((code as u32, $($t::default()),*)),
                    Err(ExecutionError::Fatal(e)) => Err(trap_from_error(e)),
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
                fn into<K: Kernel>(self, _k: &mut K) -> Result<Self::Value, Trap> {
                    match self {
                        Ok(v) => Ok((ExitCode::Ok as u32, v)),
                        Err(ExecutionError::Syscall(SyscallError(_msg, code))) => Ok((code as u32, Default::default())),
                        Err(ExecutionError::Fatal(e)) => Err(trap_from_error(e)),
                    }
                }
            }
        )*
    };
}
impl_bind_syscalls_single_return!(u32 i32 u64 i64 f32 f64);

pub fn trap_from_error(e: anyhow::Error) -> Trap {
    Trap::from(
        Box::new(ErrorEnvelope::wrap(e)) as Box<dyn std::error::Error + Send + Sync + 'static>
    )
}

#[allow(unused)]
pub fn trap_from_code(code: ExitCode) -> Trap {
    Trap::i32_exit(code as i32)
}

/// Unwraps a trap error from an actor into one of:
///
/// 1. A receipt with an exit code (if the trap is an exit).
/// 2. An "illegal actor" syscall error if the trap is caused by a WASM error.
/// 3. A fatal error otherwise.
pub fn unwrap_trap(e: Trap) -> crate::kernel::Result<Receipt> {
    use std::error::Error;

    if let Some(status) = e.i32_exit_status() {
        return Ok(Receipt {
            exit_code: ExitCode::from_i32(status)
                .with_context(|| format!("invalid exit code: {}", status))
                .or_fatal()?,
            gas_used: 0,
            return_data: Default::default(),
        });
    }

    if e.trap_code().is_some() {
        return Err(SyscallError(e.to_string(), ExitCode::SysErrIllegalActor).into());
    }

    // Do whatever we can to pull the original error back out (if it exists).
    Err(ExecutionError::Fatal(
        e.source()
            .and_then(|e| e.downcast_ref::<ErrorEnvelope>())
            .and_then(|e| e.inner.lock().ok())
            .and_then(|mut e| e.take())
            .unwrap_or_else(|| e.into()),
    ))
}

/// A super special secret error type for stapling an error to a trap in a way that allows us to
/// pull it back out.
///
/// BE VERY CAREFUL WITH THIS ERROR TYPE: Its source is self-referential.
#[derive(Display, Debug)]
#[display(fmt = "wrapping error")]
struct ErrorEnvelope {
    inner: Mutex<Option<anyhow::Error>>,
}

impl ErrorEnvelope {
    fn wrap(e: anyhow::Error) -> Self {
        Self {
            inner: Mutex::new(Some(e)),
        }
    }
}

impl std::error::Error for ErrorEnvelope {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self)
    }
}
