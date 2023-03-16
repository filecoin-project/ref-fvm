// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use actors_v10_runtime::runtime::builtins::Type;
use fvm_sdk as sdk;
use fvm_shared::address::{Address, SECP_PUB_LEN};
use fvm_shared::error::ErrorNumber;

#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    sdk::initialize();

    let method = sdk::message::method_number();

    match method {
        // our actor ID is allowed to call create actor
        1 => {
            // verify we can create a MultiSig actor without "delegated" address
            //
            let msig_addr = Address::new_id(1000);
            let msig_cid = sdk::actor::get_code_cid_for_type(Type::Multisig as i32);
            sdk::actor::create_actor(msig_addr.id().unwrap(), &msig_cid, None).unwrap();
            assert_eq!(
                None,
                sdk::actor::lookup_delegated_address(msig_addr.id().unwrap())
            );
            assert_eq!(
                msig_cid,
                sdk::actor::get_actor_code_cid(&msig_addr).unwrap()
            );

            // verify we can create an Account actor with "delegated" address
            //
            let acct_addr = Address::new_id(1001);
            let acct_cid = sdk::actor::get_code_cid_for_type(Type::Account as i32);
            let dlg_addr = Address::new_secp256k1(&[0u8; SECP_PUB_LEN]).unwrap();
            sdk::actor::create_actor(acct_addr.id().unwrap(), &acct_cid, Some(dlg_addr)).unwrap();
            assert_eq!(
                Some(dlg_addr),
                sdk::actor::lookup_delegated_address(acct_addr.id().unwrap())
            );
            assert_eq!(
                acct_cid,
                sdk::actor::get_actor_code_cid(&acct_addr).unwrap()
            );

            // creating a Placeholder without delegated" address should fail
            //
            let placeholder_cid = sdk::actor::get_code_cid_for_type(Type::Placeholder as i32);
            assert_eq!(
                Err(ErrorNumber::Forbidden),
                sdk::actor::create_actor(1002, &placeholder_cid, None)
            );

            // verify that resolving address returns None if address cannot be resolved
            //
            let not_found_addresss = Address::new_actor(&[0u8; SECP_PUB_LEN]);
            let res = sdk::actor::resolve_address(&not_found_addresss);
            assert_eq!(res, None);

            // verify that looking up code ID of an actor returns None if its not found
            //
            assert_eq!(None, sdk::actor::get_actor_code_cid(&Address::new_id(1919)));
        }
        // our actor ID is not allowed to call create actor
        2 => {
            // verify that creating a MultiSig actor without "delegated" address should fail
            //
            let msig_cid = sdk::actor::get_code_cid_for_type(Type::Multisig as i32);
            let res = sdk::actor::create_actor(1000, &msig_cid, None);
            assert_eq!(res, Err(ErrorNumber::Forbidden));

            // verify that creating an Account actor with "delegated" address should fail
            //
            let acct_cid = sdk::actor::get_code_cid_for_type(Type::Account as i32);
            let acct_addr = Address::new_secp256k1(&[0u8; SECP_PUB_LEN]).unwrap();
            let res = sdk::actor::create_actor(1001, &acct_cid, Some(acct_addr));
            assert_eq!(res, Err(ErrorNumber::Forbidden));
        }
        _ => {
            sdk::vm::abort(
                fvm_shared::error::ExitCode::FIRST_USER_EXIT_CODE,
                Some(format!("bad method {}", method).as_str()),
            );
        }
    }

    0
}
