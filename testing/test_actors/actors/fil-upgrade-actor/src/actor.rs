// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_ipld_encoding::ipld_block::IpldBlock;
//use fvm_ipld_encoding::CBOR;
use fvm_sdk as sdk;
use fvm_shared::address::Address;
use sdk::sys::ErrorNumber;
use serde_tuple::*;

//use sdk::sys::ErrorNumber;
//use fvm_shared::sys::BlockId;

#[derive(Serialize_tuple, Deserialize_tuple, PartialEq, Eq, Clone, Debug)]
struct Params {
    value: u64,
}

#[no_mangle]
pub fn upgrade(params: u32) -> u32 {
    // verify that the params we sent from invoke are the same as the params we got here
    let msg_params = sdk::message::params_raw(params).unwrap().unwrap();
    assert_eq!(msg_params.codec, fvm_ipld_encoding::CBOR);
    let p: Params = fvm_ipld_encoding::from_slice(msg_params.data.as_slice()).unwrap();
    sdk::debug::log(format!("upgrade:: Param value: {}", p.value).to_string());
    assert_eq!(p.value, 10101);

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
            let params = IpldBlock::serialize_cbor(&Params { value: 10101 }).unwrap();
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
