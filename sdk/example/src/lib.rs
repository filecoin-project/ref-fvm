use fvm_sdk as sdk;

use cid::Cid;

#[no_mangle]
pub fn invoke() -> Cid {
    return sdk::get_root();
}
