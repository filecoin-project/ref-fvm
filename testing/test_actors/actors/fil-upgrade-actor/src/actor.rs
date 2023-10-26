// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_ipld_encoding::{to_vec, CBOR};
use fvm_sdk as sdk;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ErrorNumber;
use fvm_shared::upgrade::UpgradeInfo;
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
            let block_id = sdk::ipld::put_block(CBOR, &to_vec(&666).unwrap()).unwrap();
            sdk::debug::log(format!(
                "[upgrade] params:1, returning block_id {}",
                block_id
            ));
            block_id
        }
        2 => {
            sdk::debug::log("[upgrade] params:2, calling sdk::vm::exit()".to_string());
            sdk::vm::exit(UPGRADE_FAILED_EXIT_CODE, None, None)
        }
        3 => {
            sdk::debug::log("[upgrade] params:3, calling upgrade within an upgrade".to_string());
            let new_code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10001)).unwrap();
            let params = IpldBlock::serialize_cbor(&SomeStruct { value: 2 }).unwrap();
            let _ = sdk::actor::upgrade_actor(&new_code_cid, params);
            unreachable!("we should never return from a successful upgrade");
        }
        4 => {
            let block_id = sdk::ipld::put_block(CBOR, &to_vec(&444).unwrap()).unwrap();
            sdk::debug::log(format!(
                "[upgrade] params:4, inside upgrade within an upgrade, returning block_id {}",
                block_id
            ));
            block_id
        }
        other => {
            panic!("unexpected value: {}", other);
        }
    }
}

#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    sdk::initialize();

    match sdk::message::method_number() {
        // test that successful calls to `upgrade_actor` does not return
        1 => {
            let new_code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10001)).unwrap();
            let params = IpldBlock::serialize_cbor(&SomeStruct { value: 1 }).unwrap();
            let _ = sdk::actor::upgrade_actor(&new_code_cid, params);
            unreachable!("we should never return from a successful upgrade");
        }
        // test that when `upgrade` endpoint rejects upgrade that we get the returned exit code
        2 => {
            let new_code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10000)).unwrap();
            let params = IpldBlock::serialize_cbor(&SomeStruct { value: 2 }).unwrap();
            let res = sdk::actor::upgrade_actor(&new_code_cid, params).unwrap();
            assert_eq!(
                UPGRADE_FAILED_EXIT_CODE,
                res.exit_code.value(),
                "invalid exit code returned from upgrade_actor"
            );
        }
        // test recursive update
        3 => {
            let new_code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10000)).unwrap();
            let params = IpldBlock::serialize_cbor(&SomeStruct { value: 3 }).unwrap();
            let _ = sdk::actor::upgrade_actor(&new_code_cid, params);
            unreachable!("we should never return from a successful upgrade");
        }
        // test sending a message to ourself (putting us on the call stack)
        4 => {
            sdk::send::send(
                &Address::new_id(10000),
                99,
                Default::default(),
                TokenAmount::from_atto(100),
                None,
                Default::default(),
            )
            .unwrap();
        }
        // test that calling an upgrade after self destruct fails with IllegalOperation
        5 => {
            let new_code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10000)).unwrap();
            sdk::sself::self_destruct(true).unwrap();
            let res = sdk::actor::upgrade_actor(&new_code_cid, None);
            assert_eq!(res, Err(ErrorNumber::IllegalOperation));
        }
        // test that calling an upgrade with actor already on the call stack fails
        99 => {
            let new_code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10000)).unwrap();
            let res = sdk::actor::upgrade_actor(&new_code_cid, None);
            assert_eq!(res, Err(ErrorNumber::Forbidden));
        }

        other => {
            sdk::vm::abort(
                fvm_shared::error::ExitCode::FIRST_USER_EXIT_CODE,
                Some(format!("unexpected method {}", other).as_str()),
            );
        }
    }

    0
}
