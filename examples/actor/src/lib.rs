use fvm_sdk as sdk;

/// Invoke is the actor entry point. It takes the ID of the parameters block, and returns the ID of
/// the return value block.
// NOTE: for now, the params will always just be 1. But in theory, this doesn't have to be the case.
#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    //let root = sdk::ipld::get_root();
    let _value = sdk::network::base_fee();
    sdk::ipld::UNIT
}
