use fvm_ipld_encoding::de::{Deserialize, IntoDeserializer};
use fvm_ipld_encoding::serde::Serialize;
use fvm_ipld_encoding::serde_bytes::Bytes;
use fvm_ipld_encoding::{Cbor, RawBytes, DAG_CBOR};
use fvm_sdk as sdk;
use fvm_shared::econ::TokenAmount;
use fvm_shared::message::params::ValidateParams;
use fvm_shared::sys::out::validate::GasSpec;

include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

/// Placeholder invoke for testing
#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    // Conduct method dispatch. Handle input parameters and return data.
    sdk::vm::abort(
        fvm_shared::error::ExitCode::FIRST_USER_EXIT_CODE,
        Some("sample abort"),
    )
}

/// Placeholder invoke for testing
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
