// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::ptr;

use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_ipld_encoding::IPLD_RAW;
use fvm_sdk as sdk;
use fvm_shared::address::Address;
use fvm_shared::bigint::Zero;
use fvm_shared::error::ErrorNumber::*;
use fvm_shared::error::ExitCode;
use fvm_shared::event::{Entry, Flags};

#[no_mangle]
pub fn invoke(params: u32) -> u32 {
    sdk::initialize();

    const EMIT_SEVERAL_OK: u64 = 2;
    const EMIT_MALFORMED: u64 = 3;
    const EMIT_SUBCALLS: u64 = 4;
    const EMIT_SUBCALLS_REVERT: u64 = 5;

    let payload1 = "abc".as_bytes();
    let payload2 = "def".as_bytes();
    let payload3 = "123456789 abcdefg 123456789".as_bytes();

    // Emit a single-entry event.
    let single_entry_evt = vec![Entry {
        flags: Flags::all(),
        key: "foo".to_owned(),
        codec: IPLD_RAW,
        value: payload1.to_owned(),
    }];

    let multi_entry = vec![
        Entry {
            flags: Flags::all(),
            key: "bar".to_owned(),
            codec: IPLD_RAW,
            value: payload2.to_owned(),
        },
        Entry {
            flags: Flags::FLAG_INDEXED_KEY | Flags::FLAG_INDEXED_VALUE,
            key: "👱".to_string(),
            codec: IPLD_RAW,
            value: payload3.to_owned(),
        },
    ];

    match sdk::message::method_number() {
        EMIT_SEVERAL_OK => {
            sdk::event::emit_event(&single_entry_evt.into()).unwrap();
            sdk::event::emit_event(&multi_entry.into()).unwrap();
        }
        EMIT_MALFORMED => unsafe {
            // Trigger an out of bounds.
            let entry = fvm_shared::sys::EventEntry {
                flags: Flags::empty(),
                codec: IPLD_RAW,
                key_len: 5,
                val_len: 0,
            };
            assert_eq!(
                sdk::sys::event::emit_event(
                    &entry as *const fvm_shared::sys::EventEntry,
                    1,
                    ptr::null(),
                    4,
                    ptr::null(),
                    0,
                )
                .unwrap_err(),
                IllegalArgument,
                "expected failed syscall"
            );

            // Illegal Codec
            let entry = fvm_shared::sys::EventEntry {
                flags: Flags::empty(),
                codec: 0x95,
                key_len: 0,
                val_len: 0,
            };
            assert_eq!(
                sdk::sys::event::emit_event(
                    &entry as *const fvm_shared::sys::EventEntry,
                    1,
                    ptr::null(),
                    0,
                    ptr::null(),
                    0,
                )
                .unwrap_err(),
                IllegalCodec,
                "expected failed syscall"
            );

            let buf = [0; 100];

            // Value buffer not consumed.
            let entry = fvm_shared::sys::EventEntry {
                flags: Flags::empty(),
                codec: IPLD_RAW,
                key_len: 0,
                val_len: 5,
            };
            assert_eq!(
                sdk::sys::event::emit_event(
                    &entry as *const fvm_shared::sys::EventEntry,
                    1,
                    ptr::null(),
                    0,
                    buf.as_ptr(),
                    buf.len() as u32,
                )
                .unwrap_err(),
                IllegalArgument,
                "expected failed syscall"
            );

            // Keys buffer not consumed.
            let entry = fvm_shared::sys::EventEntry {
                flags: Flags::empty(),
                codec: IPLD_RAW,
                key_len: 5,
                val_len: 0,
            };
            assert_eq!(
                sdk::sys::event::emit_event(
                    &entry as *const fvm_shared::sys::EventEntry,
                    1,
                    buf.as_ptr(),
                    buf.len() as u32,
                    ptr::null(),
                    0,
                )
                .unwrap_err(),
                IllegalArgument,
                "expected failed syscall"
            );

            // Key too large.
            let entry = fvm_shared::sys::EventEntry {
                flags: Flags::empty(),
                codec: IPLD_RAW,
                key_len: 32,
                val_len: 0,
            };
            assert_eq!(
                sdk::sys::event::emit_event(
                    &entry as *const fvm_shared::sys::EventEntry,
                    1,
                    ptr::null(),
                    0,
                    buf.as_ptr(),
                    buf.len() as u32,
                )
                .unwrap_err(),
                LimitExceeded,
                "expected failed syscall"
            );

            // Value too large.
            let entry = fvm_shared::sys::EventEntry {
                flags: Flags::empty(),
                codec: IPLD_RAW,
                key_len: 0,
                val_len: 0,
            };
            assert_eq!(
                sdk::sys::event::emit_event(
                    &entry as *const fvm_shared::sys::EventEntry,
                    1,
                    ptr::null(),
                    0,
                    buf.as_ptr(),
                    8192 + 1,
                )
                .unwrap_err(),
                LimitExceeded,
                "expected failed syscall"
            );

            // Invalid utf8.
            let emoji_key = "🧑";

            // Partial code.
            let entry = fvm_shared::sys::EventEntry {
                flags: Flags::empty(),
                codec: IPLD_RAW,
                key_len: 1,
                val_len: 0,
            };
            assert_eq!(
                sdk::sys::event::emit_event(
                    &entry as *const fvm_shared::sys::EventEntry,
                    1,
                    emoji_key.as_ptr(),
                    1,
                    ptr::null(),
                    0,
                )
                .unwrap_err(),
                IllegalArgument,
                "expected failed syscall"
            );
            // Correct utf8 but invalid boundaries.
            let entries = [
                fvm_shared::sys::EventEntry {
                    flags: Flags::empty(),
                    codec: IPLD_RAW,
                    key_len: 1,
                    val_len: 0,
                },
                fvm_shared::sys::EventEntry {
                    flags: Flags::empty(),
                    codec: IPLD_RAW,
                    key_len: emoji_key.len() as u32 - 1,
                    val_len: 0,
                },
            ];
            assert_eq!(
                sdk::sys::event::emit_event(
                    entries.as_ptr(),
                    2,
                    emoji_key.as_ptr(),
                    emoji_key.len() as u32,
                    ptr::null(),
                    0,
                )
                .unwrap_err(),
                IllegalArgument,
                "expected failed syscall"
            );
        },
        EMIT_SUBCALLS => {
            let msg_params = sdk::message::params_raw(params).unwrap().unwrap();
            assert_eq!(msg_params.codec, fvm_ipld_encoding::CBOR);

            let mut counter: u64 = fvm_ipld_encoding::from_slice(msg_params.data.as_slice())
                .expect("failed to deserialize param");

            counter -= 1;

            // emit two events.
            sdk::event::emit_event(&single_entry_evt.clone().into()).unwrap();
            sdk::event::emit_event(&single_entry_evt.into()).unwrap();

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
            assert_eq!(msg_params.codec, fvm_ipld_encoding::CBOR);

            let mut counter: u64 =
                fvm_ipld_encoding::from_slice(msg_params.data.as_slice()).unwrap();

            counter -= 1;

            // emit two events.
            sdk::event::emit_event(&single_entry_evt.clone().into()).unwrap();
            sdk::event::emit_event(&single_entry_evt.into()).unwrap();

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
