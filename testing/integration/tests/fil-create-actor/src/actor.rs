// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use actors_v10_runtime::runtime::builtins::Type;
use fvm_sdk as sdk;
use fvm_shared::address::{Address, SECP_PUB_LEN};
use fvm_shared::error::ErrorNumber;

#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    let msig_cid = sdk::actor::get_code_cid_for_type(Type::Multisig as i32);
    let acct_cid = sdk::actor::get_code_cid_for_type(Type::Account as i32);
    let acct_addr = Address::new_secp256k1(&[0u8; SECP_PUB_LEN]).unwrap();

    // Deploy
    sdk::actor::create_actor(1000, &msig_cid, None).unwrap();
    sdk::actor::create_actor(1001, &acct_cid, Some(acct_addr)).unwrap();

    // Check addresses
    assert_eq!(None, sdk::actor::lookup_delegated_address(1000));
    assert_eq!(Some(acct_addr), sdk::actor::lookup_delegated_address(1001));

    // Check code
    assert_eq!(
        msig_cid,
        sdk::actor::get_actor_code_cid(&Address::new_id(1000)).unwrap()
    );
    assert_eq!(
        acct_cid,
        sdk::actor::get_actor_code_cid(&Address::new_id(1001)).unwrap()
    );

    // Check that we can't explicitly deploy a placeholder.
    let placeholder_cid = sdk::actor::get_code_cid_for_type(Type::Placeholder as i32);
    assert_eq!(
        sdk::actor::create_actor(1002, &placeholder_cid, None),
        Err(ErrorNumber::Forbidden)
    );

    0
}
