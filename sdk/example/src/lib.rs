use fvm_sdk as sdk;

#[no_mangle]
pub fn invoke() {
    let root = sdk::get_root();
    println!("{}", root.codec());
}
