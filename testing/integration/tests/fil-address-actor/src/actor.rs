// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_ipld_encoding::RawBytes;
use fvm_sdk as sdk;
use fvm_shared::address::{Address, SECP_PUB_LEN};
use fvm_shared::bigint::Zero;
use fvm_shared::error::ExitCode;
use sdk::sys::ErrorNumber;

#[no_mangle]
pub fn invoke(params: u32) -> u32 {
    sdk::initialize();

    // Check our address.
    let (codec, data) = sdk::message::params_raw(params).unwrap();
    assert_eq!(codec, fvm_ipld_encoding::DAG_CBOR);

    match sdk::message::method_number() {
        // on construction, make sure the address matches the expected one.`
        1 => {
            let expected_address: Option<Address> = fvm_ipld_encoding::from_slice(&data).unwrap();
            let actual_address = sdk::actor::lookup_delegated_address(sdk::message::receiver());
            assert_eq!(expected_address, actual_address, "addresses did not match");
        }
        // send to an f1, then resolve.
        2 => {
            // Create an account.
            let addr = Address::new_secp256k1(&[0; SECP_PUB_LEN]).unwrap();
            assert!(sdk::send::send(
                &addr,
                0,
                RawBytes::default(),
                Zero::zero(),
                None,
                Default::default()
            )
            .unwrap()
            .exit_code
            .is_success());

            // Resolve the ID address of the account.
            let id = sdk::actor::resolve_address(&addr).expect("failed to find new account");

            assert!(
                sdk::actor::lookup_delegated_address(id).is_none(),
                "did not expect a delegated address to be assigned"
            );
        }
        // send to an f4 in the EAM's namespace, then resolve.
        3 => {
            // Create an embryo.
            let addr =
                Address::new_delegated(10, b"foobar").expect("failed to construct f4 address");
            assert!(sdk::send::send(
                &addr,
                0,
                RawBytes::default(),
                Zero::zero(),
                None,
                Default::default()
            )
            .unwrap()
            .exit_code
            .is_success());

            // Resolve the ID address of the embryo.
            let id = sdk::actor::resolve_address(&addr).expect("failed to find new embryo");

            // Lookup the address of the account.
            let new_addr =
                sdk::actor::lookup_delegated_address(id).expect("failed to lookup account address");
            assert_eq!(addr, new_addr, "addresses don't match");
        }
        // send to an f4 of an unassigned ID address, then resolve.
        4 => {
            // Create an embryo.
            let addr =
                Address::new_delegated(999, b"foobar").expect("failed to construct f4 address");
            assert_eq!(
                Err(ErrorNumber::NotFound),
                sdk::send::send(
                    &addr,
                    0,
                    RawBytes::default(),
                    Zero::zero(),
                    None,
                    Default::default()
                ),
                "expected send to unassignable f4 address to fail"
            );
        }
        // check the system actor's delegated address (should not exist).
        5 => {
            assert!(
                sdk::actor::lookup_delegated_address(0).is_none(),
                "system actor shouldn't have a 'delegated' address"
            );
        }
        _ => sdk::vm::abort(
            ExitCode::USR_UNHANDLED_MESSAGE.value(),
            Some("unknown method number"),
        ),
    }
    0
}
