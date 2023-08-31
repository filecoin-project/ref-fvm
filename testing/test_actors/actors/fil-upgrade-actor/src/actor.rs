// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_ipld_encoding::ipld_block::IpldBlock;
//use fvm_ipld_encoding::CBOR;
use fvm_sdk as sdk;
use fvm_shared::address::Address;
//use sdk::sys::ErrorNumber;
use fvm_shared::upgrade::UpgradeInfo;
use serde_tuple::*;

//use sdk::sys::ErrorNumber;
//use fvm_shared::sys::BlockId;

#[derive(Serialize_tuple, Deserialize_tuple, PartialEq, Eq, Clone, Debug)]
struct SomeStruct {
    value: u64,
}

#[no_mangle]
pub fn upgrade(params_id: u32, upgrade_info_id: u32) -> u32 {
    // verify that the params we sent from invoke are the same as the params we got here
    let params = sdk::message::params_raw(params_id).unwrap().unwrap();
    assert_eq!(params.codec, fvm_ipld_encoding::CBOR);
    let p = params.deserialize::<SomeStruct>().unwrap();
    //let p: UpgradeParams = fvm_ipld_encoding::from_slice(params.data.as_slice()).unwrap();
    sdk::debug::log(format!("upgrade:: Param value: {}", p.value).to_string());
    assert_eq!(p.value, 10101);

    // verify that the params we sent from invoke are the same as the params we got here
    let ui_params = sdk::message::params_raw(upgrade_info_id).unwrap().unwrap();
    assert_eq!(ui_params.codec, fvm_ipld_encoding::CBOR);
    let ui = ui_params.deserialize::<UpgradeInfo>().unwrap();
    //let p: Params = fvm_ipld_encoding::from_slice(msg_params.data.as_slice()).unwrap();
    sdk::debug::log(format!("upgrade: old_code_cid: {}", ui.old_code_cid).to_string());

    0
}

#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    sdk::initialize();

    let method = sdk::message::method_number();
    sdk::debug::log(format!("called upgrade_actor with method: {}", method).to_string());

    match method {
        // test that calling `upgrade_actor` on ourselves results will not return
        1 => {
            let new_code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10000)).unwrap();
            let params = IpldBlock::serialize_cbor(&SomeStruct { value: 10101 }).unwrap();
            let _ = sdk::actor::upgrade_actor(new_code_cid, params);
            assert!(false, "we should never return from a successful upgrade");
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
