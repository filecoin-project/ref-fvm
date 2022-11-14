use fvm_ipld_encoding::Cbor;
use fvm_sdk as sdk;
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
pub fn invoke(_params: u32) -> u32 {
    sdk::initialize();

    const EMIT_SEVERAL_OK: u64 = 2;
    const EMIT_MALFORMED: u64 = 3;

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
        _ => panic!("invalid method number"),
    }
    0
}
