// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_ipld_encoding::IPLD_RAW;
use fvm_sdk as sdk;
use fvm_shared::address::Address;
use fvm_shared::bigint::Zero;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::event::{Entry, Flags};
use serde_tuple::*;

#[derive(Serialize_tuple, Deserialize_tuple, PartialEq, Eq, Clone, Debug)]
struct Params {
    dest: Address,
    inner_gas_limit: u64,
    exhaust: bool,
    expect_err: bool,
}

#[no_mangle]
pub fn invoke(params_id: u32) -> u32 {
    sdk::initialize();

    let self_addr = Address::new_id(sdk::message::receiver());
    let ten = TokenAmount::from_atto(10);
    let fifty = TokenAmount::from_atto(50);

    // Gas limit to use is supplied as a param.
    let params: Params = {
        let msg_params = sdk::message::params_raw(params_id).unwrap().unwrap();
        assert_eq!(msg_params.codec, fvm_ipld_encoding::CBOR);
        fvm_ipld_encoding::from_slice(msg_params.data.as_slice()).unwrap()
    };

    // If we're self-calling, send to the origin.
    if Address::new_id(sdk::message::caller()) == self_addr {
        // Check that we successfully lowered the gas limit.
        if params.inner_gas_limit > 0 {
            assert!(sdk::gas::available() <= params.inner_gas_limit);
        }

        // This send will never be committed if we exhaust gas.
        sdk::send::send(
            &params.dest,
            0,
            Default::default(),
            ten.clone(),
            None,
            Default::default(),
        )
        .unwrap();

        // This event is also discarded if we exhaust gas.
        let single_entry_evt = {
            let payload: u64 = 400;
            vec![Entry {
                flags: Flags::all(),
                key: "foo".to_owned(),
                codec: IPLD_RAW,
                value: fvm_ipld_encoding::to_vec(&payload).unwrap().into(),
            }]
        };
        sdk::event::emit_event(&single_entry_evt.into()).unwrap();

        // Conditionally exhaust gas.
        if params.exhaust {
            let mut _i = 0;
            loop {
                _i += 1
            }
        }

        return 0;
    }

    // Send 10 to origin. This send is always persisted.
    sdk::send::send(
        &params.dest,
        0,
        Default::default(),
        fifty.clone(),
        None,
        Default::default(),
    )
    .unwrap();

    let gas_limit = if params.inner_gas_limit == 0 {
        None
    } else {
        Some(params.inner_gas_limit)
    };

    // send to self with the supplied gas_limit, propagating params.
    let msg_params = sdk::message::params_raw(params_id).unwrap();
    let ret = sdk::send::send(
        &self_addr,
        2,
        msg_params,
        Zero::zero(),
        gas_limit,
        Default::default(),
    );

    match ret {
        Ok(res) => {
            if params.expect_err {
                assert_eq!(
                    res.exit_code,
                    ExitCode::SYS_OUT_OF_GAS,
                    "expected to fail SYS_OUT_OF_GAS"
                );
            } else {
                assert!(res.exit_code.is_success(), "did not expect a failure");
            }
        }
        Err(_) => {
            panic!("did not expect an error");
        }
    };
    0
}
