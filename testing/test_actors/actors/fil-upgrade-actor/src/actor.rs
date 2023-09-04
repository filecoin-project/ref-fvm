// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_ipld_encoding::{to_vec, CBOR};
use fvm_sdk as sdk;
use fvm_shared::address::Address;
use fvm_shared::upgrade::UpgradeInfo;
use serde_tuple::*;

#[derive(Serialize_tuple, Deserialize_tuple, PartialEq, Eq, Clone, Debug)]
struct SomeStruct {
    value: u64,
}

const PARAM_1_VALUE: u64 = 111111;
const PARAM_2_VALUE: u64 = 222222;

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

    sdk::debug::log(
        format!(
            "[upgrade] value: {}, old_code_cid: {}",
            p.value, ui.old_code_cid
        )
        .to_string(),
    );

    match p.value {
        PARAM_1_VALUE => {
            sdk::debug::log("returning 0 to mark that the upgrade was successful".to_string());
            sdk::ipld::put_block(CBOR, &to_vec(&666).unwrap()).unwrap()
        }
        PARAM_2_VALUE => {
            sdk::debug::log("calling exit to mark that the upgrade failed".to_string());
            sdk::vm::exit(UPGRADE_FAILED_EXIT_CODE, None, None)
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
    sdk::debug::log(format!("called upgrade_actor with method: {}", method).to_string());

    match method {
        // test that successful calls to `upgrade_actor` does not return
        1 => {
            let new_code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10000)).unwrap();
            let params = IpldBlock::serialize_cbor(&SomeStruct {
                value: PARAM_1_VALUE,
            })
            .unwrap();
            let _ = sdk::actor::upgrade_actor(new_code_cid, params);
            assert!(false, "we should never return from a successful upgrade");
        }
        // test that when `upgrade` endpoint rejects upgrade that we get the returned exit code
        2 => {
            let new_code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10000)).unwrap();
            let params = IpldBlock::serialize_cbor(&SomeStruct {
                value: PARAM_2_VALUE,
            })
            .unwrap();
            let exit_code = sdk::actor::upgrade_actor(new_code_cid, params).unwrap();
            assert_eq!(
                UPGRADE_FAILED_EXIT_CODE, exit_code,
                "invalid exit code returned from upgrade_actor"
            );
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
