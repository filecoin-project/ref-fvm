use fvm_sdk as sdk;

#[no_mangle]
pub fn invoke() {
    let root = sdk::ipld::get_root();
    println!("{}", root.codec());
}
