// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
use fvm_ipld_encoding::{to_vec, DAG_CBOR};
use fvm_sdk as sdk;
use fvm_shared::address::Address;
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

    // test that we can't destroy the calling actor when supplied beneficiary
    // address does not exist or when its itself
    //
    assert_eq!(
        sdk::sself::self_destruct(&Address::new_id(191919)),
        Err(ActorDeleteError::BeneficiaryDoesNotExist),
    );
    assert_eq!(
        sdk::sself::self_destruct(&Address::new_id(10000)),
        Err(ActorDeleteError::BeneficiaryIsSelf),
    );

    // now lets destroy the calling actor
    //
    sdk::sself::self_destruct(&Address::new_id(sdk::message::origin())).unwrap();

    // test that root/set_root/self_destruct fail when the actor has been deleted
    // and balance is 0
    assert_eq!(sdk::sself::root().unwrap_err(), StateReadError);
    assert_eq!(
        sdk::sself::set_root(&cid).unwrap_err(),
        StateUpdateError::ActorDeleted
    );
    assert_eq!(TokenAmount::from_nano(0), sdk::sself::current_balance());

    // calling destroy on an already destroyed actor should succeed (since its
    // balance is 0)
    //
    // TODO (fridrik): we should consider changing this behaviour in the future
    // and disallow destroying actor with non-zero balance)
    //
    sdk::sself::self_destruct(&Address::new_id(sdk::message::origin()))
        .expect("deleting an already deleted actor should succeed since it has zero balance");

    #[cfg(coverage)]
    sdk::debug::store_artifact("sself_actor.profraw", minicov::capture_coverage());

    0
}
