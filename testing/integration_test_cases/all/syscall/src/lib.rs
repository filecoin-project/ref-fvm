#![feature(libstd_sys_internals)]

use std::ptr;

use fvm_ipld_encoding::tuple::{Deserialize_tuple, Serialize_tuple};
use fvm_sdk::vm::abort;

use crate::syscall::do_not_exist;

mod syscall;

/// The state object.
#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug, Default)]
pub struct State {
    empty: bool,
}

#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    // Conduct method dispatch. Handle input parameters and return data.
    match fvm_sdk::message::method_number() {
        1 => unsafe { do_not_exist(0, ptr::null(), 0) },
        _ => abort(22, Some("unrecognized method")),
    }
}
