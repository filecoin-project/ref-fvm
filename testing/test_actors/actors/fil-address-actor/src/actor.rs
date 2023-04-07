// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_sdk as sdk;
use fvm_shared::address::{Address, SECP_PUB_LEN};
use fvm_shared::bigint::Zero;
use fvm_shared::error::ExitCode;
use sdk::sys::ErrorNumber;

#[no_mangle]
pub fn invoke(params: u32) -> u32 {
    sdk::initialize();

    match sdk::message::method_number() {
        // on construction, make sure the address matches the expected one.`
        1 => {
            // Check our address.
            let msg_params = sdk::message::params_raw(params).unwrap().unwrap();
            assert_eq!(msg_params.codec, fvm_ipld_encoding::CBOR);
            let expected_address: Option<Address> =
                fvm_ipld_encoding::from_slice(msg_params.data.as_slice()).unwrap();
            let actual_address = sdk::actor::lookup_delegated_address(sdk::message::receiver());
            assert_eq!(expected_address, actual_address, "addresses did not match");
        }
        // send to an f1, then resolve.
        2 => {
            // Create an account.
            let addr = Address::new_secp256k1(&[0; SECP_PUB_LEN]).unwrap();
            assert!(
                sdk::send::send(&addr, 0, None, Zero::zero(), None, Default::default())
                    .unwrap()
                    .exit_code
                    .is_success()
            );

            // Resolve the ID address of the account.
            let id = sdk::actor::resolve_address(&addr).expect("failed to find new account");

            assert!(
                sdk::actor::lookup_delegated_address(id).is_none(),
                "did not expect a delegated address to be assigned"
            );
        }
        // send to an f4 in the EAM's namespace, then resolve.
        3 => {
            // Create a placeholder.
            let addr =
                Address::new_delegated(10, b"foobar").expect("failed to construct f4 address");
            assert!(
                sdk::send::send(&addr, 0, None, Zero::zero(), None, Default::default())
                    .unwrap()
                    .exit_code
                    .is_success()
            );

            // Resolve the ID address of the placeholder.
            let id = sdk::actor::resolve_address(&addr).expect("failed to find new placeholder");

            // Lookup the address of the account.
            let new_addr =
                sdk::actor::lookup_delegated_address(id).expect("failed to lookup account address");
            assert_eq!(addr, new_addr, "addresses don't match");
        }
        // send to an f4 of an unassigned ID address, then resolve.
        4 => {
            // Create a placeholder.
            let addr =
                Address::new_delegated(999, b"foobar").expect("failed to construct f4 address");
            assert_eq!(
                Err(ErrorNumber::NotFound),
                sdk::send::send(&addr, 0, None, Zero::zero(), None, Default::default()),
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
