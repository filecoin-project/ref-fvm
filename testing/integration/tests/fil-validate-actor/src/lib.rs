#[cfg(target_arch = "wasm32")]
use {
    fvm_ipld_encoding::{RawBytes, DAG_CBOR},
    fvm_sdk as sdk,
    fvm_shared::econ::TokenAmount,
    fvm_shared::message::params::ValidateParams,
    fvm_shared::sys::out::validate::GasSpec,
};

#[cfg(not(target_arch = "wasm32"))]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

/// Placeholder invoke for testing
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    // Conduct method dispatch. Handle input parameters and return data.
    sdk::vm::abort(
        fvm_shared::error::ExitCode::FIRST_USER_EXIT_CODE,
        Some("sample abort"),
    )
}

/// Placeholder invoke for testing
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub fn validate(msg: u32) -> u32 {
    let msg = sdk::ipld::get_block(msg, None).unwrap();

    let msg = RawBytes::from(msg).deserialize::<ValidateParams>().unwrap();

    println!("AAAA {msg:?}");
    // Conduct method dispatch. Handle input parameters and return data.
    // sdk::vm::abort(
    //     fvm_shared::error::ExitCode::FIRST_USER_EXIT_CODE,
    //     Some("sample abort"),
    // )

    let spec = GasSpec {
        gas_limit: 1000, // TODO if we are letting this be chosen by user, are we adding that to whatever is expended here?
        gas_fee_cap: TokenAmount::from_whole(1),
        gas_premium: TokenAmount::from_atto(1),
    };

    sdk::ipld::put_block(DAG_CBOR, RawBytes::serialize(spec).unwrap().bytes())
        .expect("failed to write result")
}
