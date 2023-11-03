// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_ipld_encoding::{to_vec, CBOR};
use fvm_sdk as sdk;
use fvm_shared::address::Address;
use fvm_shared::error::ExitCode;
use fvm_shared::upgrade::UpgradeInfo;
use serde_tuple::*;

#[derive(Serialize_tuple, Deserialize_tuple, PartialEq, Eq, Clone, Debug)]
struct SomeStruct {
    value: u64,
}

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
        "[upgrade-receive-actor] value: {}, old_code_cid: {}",
        p.value, ui.old_code_cid
    ));

    match p.value {
        1 => {
            // try upgrade to a cid which should fail since it does not implement the upgrade endpoint
            let new_code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10002)).unwrap();
            let params = IpldBlock::serialize_cbor(&SomeStruct { value: 4 }).unwrap();
            let res = sdk::actor::upgrade_actor(&new_code_cid, params).unwrap();
            sdk::debug::log(format!("[upgrade-receive-actor] res: {:?}", res));
            assert_eq!(
                res.exit_code,
                ExitCode::SYS_INVALID_RECEIVER,
                "expected invalid receiver error"
            );

            let block_id = sdk::ipld::put_block(CBOR, &to_vec(&666).unwrap()).unwrap();
            sdk::debug::log(format!(
                "[upgrade] params:1, returning block_id {}",
                block_id
            ));
            block_id
        }
        2 => {
            sdk::debug::log("[upgrade-receive-actor] params:2".to_string());
            let block_id = sdk::ipld::put_block(CBOR, &to_vec(&444).unwrap()).unwrap();
            sdk::debug::log(format!(
                "[upgrade] params:2, returning block_id {}",
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
    sdk::debug::log("[upgrade-receive-actor] calling vm::exit()".to_string());
    sdk::vm::exit(1, None, None)
}
