use fvm_sdk as sdk;

#[no_mangle]
pub fn invoke() {
    return sdk::get_root();
}
