// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::mem;

use fvm_shared::error::ErrorNumber;
use fvm_shared::sys::SyscallSafe;
use wasmtime::{Caller, WasmTy};

use super::context::Memory;
use super::error::Abort;
use super::{charge_for_exec, charge_syscall_gas, update_gas_available, Context, InvocationData};
use crate::call_manager::backtrace;
use crate::kernel::{self, ExecutionError, Kernel, SyscallError};

/// A "linker" for exposing syscalls to wasm modules.
pub struct Linker<K>(pub(crate) wasmtime::Linker<InvocationData<K>>);

impl<K> Linker<K> {
    /// Link a syscall.
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
    pub fn link_syscall<Args, Ret>(
        &mut self,
        module: &'static str,
        name: &'static str,
        syscall: impl Syscall<K, Args, Ret>,
    ) -> anyhow::Result<&mut Self> {
        syscall.link(self, module, name)?;
        Ok(self)
    }
}

/// A [`Syscall`] is a function in the form `fn(Context<'_, K>, I...) -> R` where:
///
/// - `K` is the kernel type. Constrain this to the precise kernel operations you need, or even
///    a specific kernel implementation.
/// - `I...`, the syscall parameters, are 0-8 types, each one of [`u32`], [`u64`], [`i32`], or
///    [`i64`].
/// - `R` is a type implementing [`IntoControlFlow`]. This is usually one of:
///     - [`kernel::Result<T>`] or [`ControlFlow<T>`] where `T`, the return value type, is
///       [`SyscallSafe`].
///     - [`Abort`] for syscalls that only abort (revert) the currently running actor.
///
/// You generally shouldn't implement this trait yourself.
pub trait Syscall<K, Args, Ret>: Send + Sync + 'static {
    /// Link this syscall with the specified linker, module name, and function name.
    fn link(
        self,
        linker: &mut Linker<K>,
        module: &'static str,
        name: &'static str,
    ) -> anyhow::Result<()>;
}

/// ControlFlow is a general-purpose enum for returning a control-flow decision from a syscall.
pub enum ControlFlow<T> {
    /// Return a value to the actor.
    Return(T),
    /// Fail with the specified syscall error.
    Error(SyscallError),
    /// Abort the running actor (exit, out of gas, or fatal error).
    Abort(Abort),
}

impl<T> From<ExecutionError> for ControlFlow<T> {
    fn from(value: ExecutionError) -> Self {
        match value {
            ExecutionError::Syscall(err) => ControlFlow::Error(err),
            ExecutionError::Fatal(err) => ControlFlow::Abort(Abort::Fatal(err)),
            ExecutionError::OutOfGas => ControlFlow::Abort(Abort::OutOfGas),
        }
    }
}

/// The helper trait used by `Syscall` to convert kernel results with execution errors into
/// results that can be handled by the wasm vm. See the documentation on [`Syscall`] for details.
pub trait IntoControlFlow: Sized {
    type Value: SyscallSafe;
    fn into_control_flow(self) -> ControlFlow<Self::Value>;
}

/// An uninhabited type. We use this in `abort` to make sure there's no way to return without
/// returning an error.
#[derive(Copy, Clone)]
pub enum Never {}
unsafe impl SyscallSafe for Never {}

// Implementations for syscalls that always abort.
impl IntoControlFlow for Abort {
    type Value = Never;
    fn into_control_flow(self) -> ControlFlow<Self::Value> {
        ControlFlow::Abort(self)
    }
}

// Implementations for syscalls that can abort.
impl<T> IntoControlFlow for ControlFlow<T>
where
    T: SyscallSafe,
{
    type Value = T;
    fn into_control_flow(self) -> ControlFlow<Self::Value> {
        self
    }
}

// Implementations for normal syscalls.
impl<T> IntoControlFlow for kernel::Result<T>
where
    T: SyscallSafe,
{
    type Value = T;
    fn into_control_flow(self) -> ControlFlow<Self::Value> {
        match self {
            Ok(value) => ControlFlow::Return(value),
            Err(e) => match e {
                ExecutionError::Syscall(err) => ControlFlow::Error(err),
                ExecutionError::OutOfGas => ControlFlow::Abort(Abort::OutOfGas),
                ExecutionError::Fatal(err) => ControlFlow::Abort(Abort::Fatal(err)),
            },
        }
    }
}

fn memory_and_data<'a, K: Kernel>(
    caller: &'a mut Caller<'_, InvocationData<K>>,
) -> (&'a mut Memory, &'a mut InvocationData<K>) {
    let memory_handle = caller.data().memory;
    let (mem, data) = memory_handle.data_and_store_mut(caller);
    (Memory::new(mem), data)
}

// Unfortunately, we can't implement this for _all_ functions. So we implement it for functions of up to 6 arguments.
macro_rules! impl_syscall {
    ($($t:ident)*) => {
        #[allow(non_snake_case)]
        impl<$($t,)* Ret, K, Func> Syscall<K, ($($t,)*), Ret> for Func
        where
            K: Kernel,
            Func: Fn(Context<'_, K> $(, $t)*) -> Ret + Send + Sync + 'static,
            Ret: IntoControlFlow,
           $($t: WasmTy+SyscallSafe,)*
        {
            fn link(
                self,
                linker: &mut Linker<K>,
                module: &'static str,
                name: &'static str,
            ) -> anyhow::Result<()> {
                if mem::size_of::<Ret::Value>() == 0 {
                    // If we're returning a zero-sized "value", we return no value therefore and expect no out pointer.
                    linker.0.func_wrap(module, name, move |mut caller: Caller<'_, InvocationData<K>> $(, $t: $t)*| {
                        charge_for_exec(&mut caller)?;
                        charge_syscall_gas(&mut caller)?;

                        let (mut memory, data) = memory_and_data(&mut caller);

                        let ctx = Context{kernel: &mut data.kernel, memory: &mut memory};
                        let out = self(ctx $(, $t)*).into_control_flow();

                        let result = match out {
                            ControlFlow::Return(_) => {
                                log::trace!("syscall {}::{}: ok", module, name);
                                data.last_error = None;
                                Ok(0)
                            },
                            ControlFlow::Error(err) => {
                                let code = err.1;
                                log::trace!("syscall {}::{}: fail ({})", module, name, code as u32);
                                data.last_error = Some(backtrace::Cause::from_syscall(module, name, err));
                                Ok(code as u32)
                            },
                            ControlFlow::Abort(abort) => Err(abort.into()),
                        };

                        update_gas_available(&mut caller)?;

                        result
                    })?;
                } else {
                    // If we're returning an actual value, we need to write it back into the wasm module's memory.
                    linker.0.func_wrap(module, name, move |mut caller: Caller<'_, InvocationData<K>>, ret: u32 $(, $t: $t)*| {
                        charge_for_exec(&mut caller)?;
                        charge_syscall_gas(&mut caller)?;

                        let (mut memory, data) = memory_and_data(&mut caller);

                        // We need to check to make sure we can store the return value _before_ we do anything.
                        if (ret as u64) > (memory.len() as u64)
                            || memory.len() - (ret as usize) < mem::size_of::<Ret::Value>() {
                            let code = ErrorNumber::IllegalArgument;
                            data.last_error = Some(backtrace::Cause::from_syscall(module, name, SyscallError(format!("no space for return value"), code)));
                            return Ok(code as u32);
                        }

                        let ctx = Context{kernel: &mut data.kernel, memory: &mut memory};
                        let result = match self(ctx $(, $t)*).into_control_flow() {
                            ControlFlow::Return(value) => {
                                log::trace!("syscall {}::{}: ok", module, name);
                                unsafe {
                                    // We're writing into a user-specified pointer, so avoid
                                    // derefering it as it may not be aligned.
                                    (memory.as_mut_ptr().offset(ret as isize) as *mut Ret::Value).write_unaligned(value);
                                }
                                data.last_error = None;
                                Ok(0)
                            },
                            ControlFlow::Error(err) => {
                                let code = err.1;
                                log::trace!("syscall {}::{}: fail ({})", module, name, code as u32);
                                data.last_error = Some(backtrace::Cause::from_syscall(module, name, err));
                                Ok(code as u32)
                            },
                            ControlFlow::Abort(abort) => Err(abort.into()),
                        };

                        update_gas_available(&mut caller)?;

                        result
                    })?;
                }
                Ok(())
            }
        }
    }
}

impl_syscall!();
impl_syscall!(A);
impl_syscall!(A B);
impl_syscall!(A B C);
impl_syscall!(A B C D);
impl_syscall!(A B C D E);
impl_syscall!(A B C D E F);
impl_syscall!(A B C D E F G);
impl_syscall!(A B C D E F G H);
