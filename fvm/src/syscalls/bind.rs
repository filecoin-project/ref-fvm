use std::mem;

use fvm_shared::error::ErrorNumber;
use fvm_shared::sys::SyscallSafe;
use wasmtime::{Caller, Linker, Trap, Val, WasmTy};

use super::context::Memory;
use super::error::Abort;
use super::{Context, InvocationData};
use crate::call_manager::backtrace;
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
    /// 1. The return type will be automatically adjusted to return `Result<u32, Trap>` where
    /// `u32` is the error code.
    /// 2. If the return type is non-empty (i.e., not `()`), an out-pointer will be prepended to the
    /// arguments for the return-value.
    ///
    /// By example:
    ///
    /// - `fn(u32) -> kernel::Result<()>` will become `fn(u32) -> Result<u32, Trap>`.
    /// - `fn(u32) -> kernel::Result<i64>` will become `fn(u32, u32) -> Result<u32, Trap>`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// mod my_module {
    ///     pub fn zero(mut context: Context<'_, impl Kernel>, arg: i32) -> crate::fvm::kernel::Result<i32> {
    ///         Ok(0)
    ///     }
    /// }
    /// let engine = wasmtime::Engine::default();
    /// let mut linker = wasmtime::Linker::new(&engine);
    /// linker.bind("my_module", "zero", my_module::zero);
    /// ```
    fn bind(
        &mut self,
        module: &'static str,
        name: &'static str,
        syscall: Func,
    ) -> anyhow::Result<&mut Self>;
}

/// The helper trait used by `BindSyscall` to convert kernel results with execution errors into
/// results that can be handled by wasmtime. See the documentation on `BindSyscall` for details.
#[doc(hidden)]
pub trait IntoSyscallResult: Sized {
    type Value: SyscallSafe;
    fn into(self) -> Result<Result<Self::Value, SyscallError>, Abort>;
}

// Implementations for syscalls that abort on error.
impl<T> IntoSyscallResult for Result<T, Abort>
where
    T: SyscallSafe,
{
    type Value = T;
    fn into(self) -> Result<Result<Self::Value, SyscallError>, Abort> {
        Ok(Ok(self?))
    }
}

// Implementations for normal syscalls.
impl<T> IntoSyscallResult for kernel::Result<T>
where
    T: SyscallSafe,
{
    type Value = T;
    fn into(self) -> Result<Result<Self::Value, SyscallError>, Abort> {
        match self {
            Ok(value) => Ok(Ok(value)),
            Err(e) => match e {
                ExecutionError::Syscall(err) => Ok(Err(err)),
                ExecutionError::OutOfGas => Err(Abort::OutOfGas),
                ExecutionError::Fatal(err) => Err(Abort::Fatal(err)),
            },
        }
    }
}

fn memory_and_data<'a, K: Kernel>(
    caller: &'a mut Caller<'_, InvocationData<K>>,
) -> Result<(&'a mut Memory, &'a mut InvocationData<K>), Trap> {
    let (mem, data) = caller
        .get_export("memory")
        .and_then(|m| m.into_memory())
        .ok_or_else(|| Trap::new("failed to lookup actor memory"))?
        .data_and_store_mut(caller);
    Ok((Memory::new(mem), data))
}

fn gastracker_to_wasmgas(caller: &mut Caller<InvocationData<impl Kernel>>) -> Result<(), Trap> {
    let avail_milligas = caller
        .data_mut()
        .kernel
        .borrow_milligas()
        .map_err(|_| Trap::new("borrowing available gas"))?;

    let gas_global = caller.data_mut().avail_gas_global;
    gas_global
        .set(caller, Val::I64(avail_milligas))
        .map_err(|_| Trap::new("failed to set available gas"))
}

fn wasmgas_to_gastracker(caller: &mut Caller<InvocationData<impl Kernel>>) -> Result<(), Trap> {
    let global = caller.data_mut().avail_gas_global;

    let milligas = match global.get(&mut *caller) {
        Val::I64(g) => Ok(g),
        _ => Err(Trap::new("failed to get wasm gas")),
    }?;

    // note: this should never error:
    // * It can't return out-of-gas, because that would mean that we got
    //   negative available milligas returned from wasm - and wasm
    //   instrumentation will trap when it sees available gas go below zero
    // * If it errors because gastracker thinks it already owns gas, something
    //   is really wrong
    caller
        .data_mut()
        .kernel
        .return_milligas("wasm_exec", milligas)
        .map_err(|e| Trap::new(format!("returning available gas: {}", e)))?;
    Ok(())
}

// Unfortunately, we can't implement this for _all_ functions. So we implement it for functions of up to 6 arguments.
macro_rules! impl_bind_syscalls {
    ($($t:ident)*) => {
        #[allow(non_snake_case)]
        impl<$($t,)* Ret, K, Func> BindSyscall<($($t,)*), Ret, Func> for Linker<InvocationData<K>>
        where
            K: Kernel,
            Func: Fn(Context<'_, K> $(, $t)*) -> Ret + Send + Sync + 'static,
            Ret: IntoSyscallResult,
           $($t: WasmTy+SyscallSafe,)*
        {
            fn bind(
                &mut self,
                module: &'static str,
                name: &'static str,
                syscall: Func,
            ) -> anyhow::Result<&mut Self> {
                if mem::size_of::<Ret::Value>() == 0 {
                    // If we're returning a zero-sized "value", we return no value therefore and expect no out pointer.
                    self.func_wrap(module, name, move |mut caller: Caller<'_, InvocationData<K>> $(, $t: $t)*| {
                        wasmgas_to_gastracker(&mut caller)?;

                        let (mut memory, mut data) = memory_and_data(&mut caller)?;
                        let ctx = Context{kernel: &mut data.kernel, memory: &mut memory};
                        let out = syscall(ctx $(, $t)*).into();

                        let result = match out {
                            Ok(Ok(_)) => {
                                log::trace!("syscall {}::{}: ok", module, name);
                                data.last_error = None;
                                Ok(0)
                            },
                            Ok(Err(err)) => {
                                let code = err.1;
                                log::trace!("syscall {}::{}: fail ({})", module, name, code as u32);
                                data.last_error = Some(backtrace::Cause::new(module, name, err));
                                Ok(code as u32)
                            },
                            Err(e) => Err(e.into()),
                        };

                        gastracker_to_wasmgas(&mut caller)?;

                        result
                    })
                } else {
                    // If we're returning an actual value, we need to write it back into the wasm module's memory.
                    self.func_wrap(module, name, move |mut caller: Caller<'_, InvocationData<K>>, ret: u32 $(, $t: $t)*| {
                        wasmgas_to_gastracker(&mut caller)?;
                        let (mut memory, mut data) = memory_and_data(&mut caller)?;

                        // We need to check to make sure we can store the return value _before_ we do anything.
                        if (ret as u64) > (memory.len() as u64)
                            || memory.len() - (ret as usize) < mem::size_of::<Ret::Value>() {
                            let code = ErrorNumber::IllegalArgument;
                            data.last_error = Some(backtrace::Cause::new(module, name, SyscallError(format!("no space for return value"), code)));
                            return Ok(code as u32);
                        }

                        let ctx = Context{kernel: &mut data.kernel, memory: &mut memory};
                        let result = match syscall(ctx $(, $t)*).into() {
                            Ok(Ok(value)) => {
                                log::trace!("syscall {}::{}: ok", module, name);
                                unsafe { *(memory.as_mut_ptr().offset(ret as isize) as *mut Ret::Value) = value };
                                data.last_error = None;
                                Ok(0)
                            },
                            Ok(Err(err)) => {
                                let code = err.1;
                                log::trace!("syscall {}::{}: fail ({})", module, name, code as u32);
                                data.last_error = Some(backtrace::Cause::new(module, name, err));
                                Ok(code as u32)
                            },
                            Err(e) => Err(e.into()),
                        };

                        gastracker_to_wasmgas(&mut caller)?;

                        result
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
