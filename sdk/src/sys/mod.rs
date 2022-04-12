//! This module defines the low-level syscall FFI "shims".
#[doc(inline)]
pub use fvm_shared::sys::TokenAmount;

pub mod actor;
pub mod crypto;
//#[cfg(feature = "debug")]
pub mod debug;
pub mod gas;
pub mod ipld;
pub mod message;
pub mod network;
pub mod rand;
pub mod send;
pub mod sself;
pub mod vm;

/// Generate a set of FVM syscall shims.
///
/// ```ignore
/// fvm_sdk::sys::fvm_syscalls! {
///     module = "my_wasm_module";
///
///     /// This method will translate to a syscall with the signature:
///     ///
///     ///     fn(arg: u64) -> u32;
///     ///
///     /// Where the returned u32 is the status code.
///     pub fn returns_nothing(arg: u64) -> Result<()>;
///
///     /// This method will translate to a syscall with the signature:
///     ///
///     ///     fn(out: u32, arg: u32) -> u32;
///     ///
///     /// Where `out` is a pointer to where the return value will be written and the returned u32
///     /// is the status code.
///     pub fn returns_value(arg: u64) -> Result<u64>;
///
///     /// This method will translate to a syscall with the signature:
///     ///
///     ///     fn(arg: u32) -> u32;
///     ///
///     /// But it will panic if this function returns.
///     pub fn aborts(arg: u32) -> !;
/// }
/// ```
macro_rules! fvm_syscalls {
    // Returns no values.
    (module = $module:literal; $(#[$attrs:meta])* $v:vis fn $name:ident($($args:ident : $args_ty:ty),*$(,)?) -> Result<()>; $($rest:tt)*) => {
        $(#[$attrs])*
        $v unsafe fn $name($($args:$args_ty),*) -> Result<(), fvm_shared::error::ErrorNumber> {
            #[link(wasm_import_module = $module)]
            extern "C" {
                #[link_name = stringify!($name)]
                fn syscall($($args:$args_ty),*) -> u32;
            }

            let code = syscall($($args),*);

            if code == 0 {
                Ok(())
            } else {
                Err(num_traits::FromPrimitive::from_u32(code)
                    .expect("syscall returned unrecognized exit code"))
            }
        }
        $crate::sys::fvm_syscalls! {
            module = $module; $($rest)*
        }
    };
    // Returns a value.
    (module = $module:literal; $(#[$attrs:meta])* $v:vis fn $name:ident($($args:ident : $args_ty:ty),*$(,)?) -> Result<$ret:ty>; $($rest:tt)*) => {
        $(#[$attrs])*
        $v unsafe fn $name($($args:$args_ty),*) -> Result<$ret, fvm_shared::error::ErrorNumber> {
            #[link(wasm_import_module = $module)]
            extern "C" {
                #[link_name = stringify!($name)]
                fn syscall(ret: *mut $ret $(, $args : $args_ty)*) -> u32;
            }

            let mut ret = std::mem::MaybeUninit::<$ret>::uninit();
            let code = syscall(ret.as_mut_ptr(), $($args),*);

            if code == 0 {
                Ok(ret.assume_init())
            } else {
                Err(num_traits::FromPrimitive::from_u32(code)
                    .expect("syscall returned unrecognized exit code"))
            }
        }
        $crate::sys::fvm_syscalls! {
            module = $module;
            $($rest)*
        }
    };
    // Does not return.
    (module = $module:literal; $(#[$attrs:meta])* $v:vis fn $name:ident($($args:ident : $args_ty:ty),*$(,)?) -> !; $($rest:tt)*) => {
        $(#[$attrs])*
        $v unsafe fn $name($($args:$args_ty),*) -> ! {
            #[link(wasm_import_module = $module)]
            extern "C" {
                #[link_name = stringify!($name)]
                fn syscall($($args : $args_ty),*) -> u32;
            }

            syscall($($args),*);
            panic!(concat!("syscall ", stringify!($name), " should not have returned"))
        }
        $crate::sys::fvm_syscalls! {
            module = $module;
            $($rest)*
        }
    };
    // Base case.
    (module = $module:literal;) => {};
}

pub(crate) use fvm_syscalls;
