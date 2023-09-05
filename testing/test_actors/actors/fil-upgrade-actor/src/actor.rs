// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::multihash::Multihash;
use cid::Cid;
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_ipld_encoding::{to_vec, CBOR};
use fvm_sdk as sdk;
use fvm_shared::address::Address;
use fvm_shared::error::ErrorNumber;
use fvm_shared::upgrade::UpgradeInfo;
use fvm_shared::IDENTITY_HASH;
use serde_tuple::*;
#[derive(Serialize_tuple, Deserialize_tuple, PartialEq, Eq, Clone, Debug)]
struct SomeStruct {
    value: u64,
}

const UPGRADE_FAILED_EXIT_CODE: u32 = 19;

#[no_mangle]
pub fn upgrade(params_id: u32, upgrade_info_id: u32) -> u32 {
    sdk::initialize();

    let params = sdk::message::params_raw(params_id).unwrap().unwrap();
    let ui_params = sdk::message::params_raw(upgrade_info_id).unwrap().unwrap();

    assert_eq!(params.codec, fvm_ipld_encoding::CBOR);
    assert_eq!(ui_params.codec, fvm_ipld_encoding::CBOR);

    let p = params.deserialize::<SomeStruct>().unwrap();
    let ui = ui_params.deserialize::<UpgradeInfo>().unwrap();

    sdk::debug::log(format!(
        "[upgrade] value: {}, old_code_cid: {}",
        p.value, ui.old_code_cid
    ));

    match p.value {
        1 => {
            sdk::debug::log("returning 0 to mark that the upgrade was successful".to_string());
            sdk::ipld::put_block(CBOR, &to_vec(&666).unwrap()).unwrap()
        }
        2 => {
            sdk::debug::log("calling exit to mark that the upgrade failed".to_string());
            sdk::vm::exit(UPGRADE_FAILED_EXIT_CODE, None, None)
        }
        3 => {
            sdk::debug::log("calling upgrade within an upgrade".to_string());
            let new_code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10000)).unwrap();
            let params = IpldBlock::serialize_cbor(&SomeStruct { value: 4 }).unwrap();
            let _ = sdk::actor::upgrade_actor(new_code_cid, params);
            unreachable!("we should never return from a successful upgrade");
        }
        4 => {
            sdk::debug::log("inside upgrade within an upgrade".to_string());
            sdk::ipld::put_block(CBOR, &to_vec(&444).unwrap()).unwrap()
        }
        _ => {
            panic!("unexpected value: {}", p.value);
        }
    }
}

#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    sdk::initialize();

    let method = sdk::message::method_number();
    sdk::debug::log(format!("called upgrade_actor with method: {}", method));

    match method {
        // test that successful calls to `upgrade_actor` does not return
        1 => {
            let new_code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10000)).unwrap();
            let params = IpldBlock::serialize_cbor(&SomeStruct { value: 1 }).unwrap();
            let _ = sdk::actor::upgrade_actor(new_code_cid, params);
            unreachable!("we should never return from a successful upgrade");
        }
        // test that when `upgrade` endpoint rejects upgrade that we get the returned exit code
        2 => {
            let new_code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10000)).unwrap();
            let params = IpldBlock::serialize_cbor(&SomeStruct { value: 2 }).unwrap();
            let exit_code = sdk::actor::upgrade_actor(new_code_cid, params).unwrap();
            assert_eq!(
                UPGRADE_FAILED_EXIT_CODE, exit_code,
                "invalid exit code returned from upgrade_actor"
            );
        }
        // test that providing invalid new_code_cid returns a NotFound error
        3 => {
            let new_code_cid =
                Cid::new_v1(0x55, Multihash::wrap(IDENTITY_HASH, b"test123").unwrap());
            let res = sdk::actor::upgrade_actor(new_code_cid, None);
            assert_eq!(res, Err(ErrorNumber::NotFound));
        }
        // test recursive updare
        4 => {
            let new_code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10000)).unwrap();
            let params = IpldBlock::serialize_cbor(&SomeStruct { value: 3 }).unwrap();
            sdk::debug::log("upgrade 1".to_string());
            let res = sdk::actor::upgrade_actor(new_code_cid, params);
            sdk::debug::log("upgrade 2".to_string());
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
