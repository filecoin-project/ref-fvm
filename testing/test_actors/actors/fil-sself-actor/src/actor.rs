// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
use fvm_ipld_encoding::{to_vec, DAG_CBOR};
use fvm_sdk as sdk;
use fvm_shared::econ::TokenAmount;
use sdk::error::{ActorDeleteError, StateReadError, StateUpdateError};

#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    sdk::initialize();

    assert!(!sdk::vm::read_only());

    // test that root() returns the correct root
    //
    let empty = to_vec::<[(); 0]>(&[]).unwrap();
    let expected_root = Cid::new_v1(DAG_CBOR, Code::Blake2b256.digest(&empty));
    let root = sdk::sself::root().unwrap();
    assert_eq!(root, expected_root);

    // test setting the root cid for the caling actor returns the correct root
    //
    let cid = sdk::ipld::put(0xb220, 32, 0x55, b"foo").unwrap();
    sdk::sself::set_root(&cid).unwrap();
    let root = sdk::sself::root().unwrap();
    assert_eq!(root, cid);

    let balance = sdk::sself::current_balance();
    assert_eq!(TokenAmount::from_nano(1_000_000), balance);

    // Now destroy the actor without burning funds. This should fail because we have unspent funds.
    assert_eq!(
        sdk::sself::self_destruct(false).unwrap_err(),
        ActorDeleteError::UnspentFunds
    );

    // Now lets destroy the actor, burning the funds.
    sdk::sself::self_destruct(true).unwrap();

    // test that root/set_root/self_destruct fail when the actor has been deleted
    // and balance is 0
    assert_eq!(sdk::sself::root().unwrap_err(), StateReadError);
    assert_eq!(
        sdk::sself::set_root(&cid).unwrap_err(),
        StateUpdateError::ActorDeleted
    );
    assert_eq!(TokenAmount::from_nano(0), sdk::sself::current_balance());

    // calling destroy on an already destroyed actor should succeed (no-op)
    sdk::sself::self_destruct(false).expect("deleting an already deleted actor should succeed");

    #[cfg(coverage)]
    sdk::debug::store_artifact("sself_actor.profraw", minicov::capture_coverage());

    0
}
