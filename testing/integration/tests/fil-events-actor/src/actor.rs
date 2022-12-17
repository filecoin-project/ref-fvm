// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_ipld_encoding::Cbor;
use fvm_sdk as sdk;
use fvm_shared::address::Address;
use fvm_shared::bigint::Zero;
use fvm_shared::error::ExitCode;
use fvm_shared::event::{Entry, Flags};
use serde_tuple::*;

#[derive(Serialize_tuple, Deserialize_tuple, PartialEq, Eq, Clone, Debug)]
struct EventPayload1 {
    a: String,
    b: String,
}

impl Cbor for EventPayload1 {}

#[derive(Serialize_tuple, Deserialize_tuple, PartialEq, Eq, Clone, Debug)]
struct EventPayload2 {
    c: i32,
    d: Vec<u64>,
}

impl Cbor for EventPayload2 {}

#[no_mangle]
pub fn invoke(params: u32) -> u32 {
    sdk::initialize();

    const EMIT_SEVERAL_OK: u64 = 2;
    const EMIT_MALFORMED: u64 = 3;
    const EMIT_SUBCALLS: u64 = 4;
    const EMIT_SUBCALLS_REVERT: u64 = 5;

    // Emit a single-entry event.
    let payload = EventPayload1 {
        a: String::from("aaa111"),
        b: String::from("bbb111"),
    };

    let single_entry_evt = vec![Entry {
        flags: Flags::all(),
        key: "foo".to_owned(),
        value: payload.marshal_cbor().unwrap().into(),
    }];

    let payload1 = EventPayload1 {
        a: String::from("aaa222"),
        b: String::from("bbb222"),
    };
    let payload2 = EventPayload2 {
        c: 42,
        d: vec![1, 2, 3, 4],
    };

    let multi_entry = vec![
        Entry {
            flags: Flags::all(),
            key: "bar".to_owned(),
            value: payload1.marshal_cbor().unwrap().into(),
        },
        Entry {
            flags: Flags::FLAG_INDEXED_KEY | Flags::FLAG_INDEXED_VALUE,
            key: "baz".to_string(),
            value: payload2.marshal_cbor().unwrap().into(),
        },
    ];

    match sdk::message::method_number() {
        EMIT_SEVERAL_OK => {
            sdk::event::emit_event(&single_entry_evt.into()).unwrap();
            sdk::event::emit_event(&multi_entry.into()).unwrap();
        }
        EMIT_MALFORMED => unsafe {
            // mangle an event.
            let mut serialized = single_entry_evt.marshal_cbor().unwrap();
            serialized[1] = 0xff;

            assert!(
                sdk::sys::event::emit_event(serialized.as_ptr(), serialized.len() as u32).is_err(),
                "expected failed syscall"
            );
        },
        EMIT_SUBCALLS => {
            let msg_params = sdk::message::params_raw(params).unwrap().unwrap();
            assert_eq!(msg_params.codec, fvm_ipld_encoding::DAG_CBOR);

            let mut counter: u64 = fvm_ipld_encoding::from_slice(msg_params.data.as_slice())
                .expect("failed to deserialize param");

            counter -= 1;

            // emit two events.
            sdk::event::emit_event(&single_entry_evt.clone().into()).unwrap();
            sdk::event::emit_event(&single_entry_evt.clone().into()).unwrap();

            let our_addr = Address::new_id(sdk::message::receiver());

            if counter > 0 {
                sdk::send::send(
                    &our_addr,
                    EMIT_SUBCALLS,
                    IpldBlock::serialize_cbor(&counter).unwrap(),
                    Zero::zero(),
                    None,
                    Default::default(),
                )
                .unwrap();
            }
        }
        EMIT_SUBCALLS_REVERT => {
            let msg_params = sdk::message::params_raw(params).unwrap().unwrap();
            assert_eq!(msg_params.codec, fvm_ipld_encoding::DAG_CBOR);

            let mut counter: u64 =
                fvm_ipld_encoding::from_slice(msg_params.data.as_slice()).unwrap();

            counter -= 1;

            // emit two events.
            sdk::event::emit_event(&single_entry_evt.clone().into()).unwrap();
            sdk::event::emit_event(&single_entry_evt.clone().into()).unwrap();

            let our_addr = Address::new_id(sdk::message::receiver());

            if counter > 0 {
                // This call will fail when performing the 6th call. We do not unwrap or propagate
                // the error here, we just ignore it and move on. That's part of the test scenario
                // (want to verify that the FVM correctly discards only events under a failing
                // callee, no more and no less)
                let _ = sdk::send::send(
                    &our_addr,
                    EMIT_SUBCALLS_REVERT,
                    IpldBlock::serialize_cbor(&counter).unwrap(),
                    Zero::zero(),
                    None,
                    Default::default(),
                )
                .ok();
            }

            // The 6th call will abort after performing its send. The caller won't rethrow, so we
            // will observe an OK externally. The events from the depth-most 4 callees + us should
            // be discarded (i.e. 10 events discarded).
            if counter == 4 {
                sdk::vm::abort(ExitCode::USR_ASSERTION_FAILED.value(), None);
            }
        }
        _ => panic!("invalid method number"),
    }
    0
}
