use std::mem;

use fvm_shared::error::ErrorNumber;
use fvm_shared::sys::SyscallSafe;
use wasmtime::{Caller, Linker, Trap, WasmTy};

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

fn charge_exec_units_for_gas(caller: &mut Caller<InvocationData<impl Kernel>>) -> Result<(), Trap> {
    let exec_units = caller
        .data_mut()
        .calculate_exec_units_for_gas()
        .map_err(|_| Trap::new("failed to calculate exec_units"))?;
    if exec_units.is_negative() {
        caller.add_fuel(u64::try_from(exec_units.saturating_neg()).unwrap_or(0))?;
    } else {
        caller.consume_fuel(u64::try_from(exec_units).unwrap_or(0))?;
    }

    let gas_used = caller.data().kernel.gas_used();
    let fuel_consumed = caller
        .fuel_consumed()
        .ok_or_else(|| Trap::new("expected to find exec_units consumed"))?;
    caller.data_mut().set_snapshots(gas_used, fuel_consumed);
    Ok(())
}

fn charge_gas_for_exec_units(caller: &mut Caller<InvocationData<impl Kernel>>) -> Result<(), Trap> {
    let exec_units_consumed = caller
        .fuel_consumed()
        .ok_or_else(|| Trap::new("expected to find exec_units consumed"))?;

    caller
        .data_mut()
        .charge_gas_for_exec_units(exec_units_consumed)
        .map_err(|_| Trap::new("failed to charge gas for exec_units"))
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
                        charge_gas_for_exec_units(&mut caller)?;
                        let (mut memory, mut data) = memory_and_data(&mut caller)?;
                        let ctx = Context{kernel: &mut data.kernel, memory: &mut memory};
                        let result = match syscall(ctx $(, $t)*).into()? {
                            Ok(_) => {
                                log::trace!("syscall {}::{}: ok", module, name);
                                data.last_error = None;
                                0
                            },
                            Err(err) => {
                                let code = err.1;
                                log::trace!("syscall {}::{}: fail ({})", module, name, code as u32);
                                data.last_error = Some(backtrace::Cause::new(module, name, err));
                                code as u32
                            },
                        };

                        charge_exec_units_for_gas(&mut caller)?;
                        Ok(result)
                    })
                } else {
                    // If we're returning an actual value, we need to write it back into the wasm module's memory.
                    self.func_wrap(module, name, move |mut caller: Caller<'_, InvocationData<K>>, ret: u32 $(, $t: $t)*| {
                        charge_gas_for_exec_units(&mut caller)?;
                        let (mut memory, mut data) = memory_and_data(&mut caller)?;

                        // We need to check to make sure we can store the return value _before_ we do anything.
                        if (ret as u64) > (memory.len() as u64)
                            || memory.len() - (ret as usize) < mem::size_of::<Ret::Value>() {
                            let code = ErrorNumber::IllegalArgument;
                            data.last_error = Some(backtrace::Cause::new(module, name, SyscallError(format!("no space for return value"), code)));
                            return Ok(code as u32);
                        }

                        let ctx = Context{kernel: &mut data.kernel, memory: &mut memory};
                        let result = match syscall(ctx $(, $t)*).into()? {
                            Ok(value) => {
                                log::trace!("syscall {}::{}: ok", module, name);
                                unsafe { *(memory.as_mut_ptr().offset(ret as isize) as *mut Ret::Value) = value };
                                data.last_error = None;
                                0
                            },
                            Err(err) => {
                                let code = err.1;
                                log::trace!("syscall {}::{}: fail ({})", module, name, code as u32);
                                data.last_error = Some(backtrace::Cause::new(module, name, err));
                                code as u32
                            },
                        };

                        charge_exec_units_for_gas(&mut caller)?;
                        Ok(result)
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
