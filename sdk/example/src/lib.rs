use fvm_sdk as sdk;

#[no_mangle]
pub fn invoke() {
    let _ = sdk::get_root();
}
