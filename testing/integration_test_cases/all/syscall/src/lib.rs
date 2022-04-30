#![feature(libstd_sys_internals)]

use std::ptr;

use fvm_ipld_encoding::tuple::{Deserialize_tuple, Serialize_tuple};
use fvm_sdk::vm::abort;
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
        // Unknown code for abort
        1 => abort(337, None),
        _ => abort(22, Some("unrecognized method")),
    }
}
