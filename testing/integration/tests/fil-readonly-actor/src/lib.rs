// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#[cfg(not(target_arch = "wasm32"))]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use cid::multihash::{Code, MultihashDigest};
use cid::Cid;
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_ipld_encoding::{to_vec, CBOR, DAG_CBOR, IPLD_RAW};
use fvm_sdk as sdk;
use fvm_shared::address::{Address, SECP_PUB_LEN};
use fvm_shared::econ::TokenAmount;
use fvm_shared::event::{Entry, Flags};
use fvm_shared::sys::SendFlags;
use fvm_shared::METHOD_SEND;
use sdk::error::{ActorDeleteError, StateUpdateError};
use sdk::sys::ErrorNumber;

/// Placeholder invoke for testing
#[no_mangle]
#[cfg(target_arch = "wasm32")]
pub fn invoke(blk: u32) -> u32 {
    invoke_method(blk, sdk::message::method_number())
}

#[allow(dead_code)]
fn invoke_method(blk: u32, method: u64) -> u32 {
    let account = Address::new_secp256k1(&[0u8; SECP_PUB_LEN]).unwrap();
    match method {
        2 => {
            assert!(!sdk::vm::read_only());
            // Can't create actors when read-only.
            let resp = sdk::send::send(
                &account,
                METHOD_SEND,
                None,
                TokenAmount::default(),
                None,
                SendFlags::READ_ONLY,
            );
            assert_eq!(resp, Err(ErrorNumber::ReadOnly));

            // But can still create them when not read-only.
            assert!(sdk::send::send(
                &account,
                METHOD_SEND,
                Default::default(),
                Default::default(),
                None,
                Default::default(),
            )
            .unwrap()
            .exit_code
            .is_success());

            // Now recurse.
            assert!(sdk::send::send(
                &Address::new_id(sdk::message::receiver()),
                3,
                Default::default(),
                Default::default(),
                None,
                SendFlags::READ_ONLY,
            )
            .unwrap()
            .exit_code
            .is_success());
        }
        3 => {
            // should now be in read-only mode.
            assert!(sdk::vm::read_only());

            // Sending value fails.
            let resp = sdk::send::send(
                &account,
                0,
                Default::default(),
                TokenAmount::from_atto(1),
                None,
                Default::default(),
            );
            assert_eq!(resp, Err(ErrorNumber::ReadOnly));

            // Sending nothing succeeds.
            assert!(sdk::send::send(
                &account,
                0,
                Default::default(),
                Default::default(),
                None,
                Default::default(),
            )
            .unwrap()
            .exit_code
            .is_success());

            // Writing should succeed.
            let cid = sdk::ipld::put(0xb220, 32, 0x55, b"foo").unwrap();

            // Setting root should fail.
            let err = sdk::sself::set_root(&cid).expect_err("successfully set root");
            assert_eq!(err, StateUpdateError::ReadOnly);

            // Root should not be updated.
            let empty = to_vec::<[(); 0]>(&[]).unwrap();
            let expected_root = Cid::new_v1(DAG_CBOR, Code::Blake2b256.digest(&empty));
            let root = sdk::sself::root().unwrap();
            assert_eq!(root, expected_root);

            // Send should be able to pass values.
            let output = sdk::send::send(
                &Address::new_id(sdk::message::receiver()),
                4,
                Some(IpldBlock {
                    codec: CBOR,
                    data: "input".into(),
                }),
                Default::default(),
                None,
                Default::default(),
            )
            .unwrap();
            assert!(output.exit_code.is_success());
            assert_eq!(output.return_data.unwrap().data, b"output");

            // Aborts should work.
            let output = sdk::send::send(
                &Address::new_id(sdk::message::receiver()),
                5,
                None,
                Default::default(),
                None,
                Default::default(),
            )
            .unwrap();
            assert_eq!(output.exit_code.value(), 42);

            // Should be able to recursivly send in read-only mode.
            let output = sdk::send::send(
                &Address::new_id(sdk::message::receiver()),
                4,
                Some(IpldBlock {
                    codec: CBOR,
                    data: "input".into(),
                }),
                Default::default(),
                None,
                SendFlags::READ_ONLY,
            )
            .unwrap();
            assert!(output.exit_code.is_success());
            assert_eq!(output.return_data.unwrap().data, b"output");

            // Should fail to emit events.
            let evt = vec![Entry {
                flags: Flags::all(),
                key: "foo".to_owned(),
                codec: IPLD_RAW,
                value: vec![0, 1, 2],
            }];
            let err = sdk::event::emit_event(&evt.into()).unwrap_err();
            assert_eq!(err, ErrorNumber::ReadOnly);

            // Should not be able to delete self.
            let err =
                sdk::sself::self_destruct(&Address::new_id(sdk::message::origin())).unwrap_err();
            assert_eq!(err, ActorDeleteError::ReadOnly);
        }
        4 => {
            assert!(sdk::vm::read_only());
            // read params and return value entirely in read-only mode.
            let input = sdk::ipld::get_block(blk, None).unwrap();
            assert_eq!(input, b"input");
            return sdk::ipld::put_block(0x55, b"output").unwrap();
        }
        5 => {
            assert!(sdk::vm::read_only());
            sdk::vm::abort(42, None)
        }
        _ => panic!("unexpected method"),
    }
    0
}
