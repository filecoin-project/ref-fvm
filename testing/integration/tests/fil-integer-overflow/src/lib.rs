mod blockstore;

use cid::multihash::Code;
use cid::Cid;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::{to_vec, CborStore, RawBytes, DAG_CBOR};
use fvm_sdk::message::params_raw;
use fvm_sdk::vm::abort;
use fvm_sdk::NO_DATA_BLOCK_ID;
use fvm_shared::error::ExitCode;

use crate::blockstore::Blockstore;

#[derive(Serialize_tuple, Deserialize_tuple, Clone, Debug, Default)]
pub struct State {
    pub value: i64,
}

impl State {
    pub fn load() -> Self {
        // First, load the current state root.
        let root = match fvm_sdk::sself::root() {
            Ok(root) => root,
            Err(err) => abort(
                ExitCode::USR_ILLEGAL_STATE.value(),
                Some(format!("failed to get root: {:?}", err).as_str()),
            ),
        };

        // Load the actor state from the state tree.
        match Blockstore.get_cbor::<Self>(&root) {
            Ok(Some(state)) => state,
            Ok(None) => abort(
                ExitCode::USR_ILLEGAL_STATE.value(),
                Some("state does not exist"),
            ),
            Err(err) => abort(
                ExitCode::USR_ILLEGAL_STATE.value(),
                Some(format!("failed to get state: {}", err).as_str()),
            ),
        }
    }

    pub fn save(&self) -> Cid {
        let serialized = match to_vec(self) {
            Ok(s) => s,
            Err(err) => abort(
                ExitCode::USR_SERIALIZATION.value(),
                Some(format!("failed to serialize state: {:?}", err).as_str()),
            ),
        };
        let cid = match fvm_sdk::ipld::put(
            Code::Blake2b256.into(),
            32,
            DAG_CBOR,
            serialized.as_slice(),
        ) {
            Ok(cid) => cid,
            Err(err) => abort(
                ExitCode::USR_SERIALIZATION.value(),
                Some(format!("failed to store initial state: {:}", err).as_str()),
            ),
        };
        if let Err(err) = fvm_sdk::sself::set_root(&cid) {
            abort(
                ExitCode::USR_ILLEGAL_STATE.value(),
                Some(format!("failed to set root ciid: {:}", err).as_str()),
            );
        }
        cid
    }
}

#[no_mangle]
pub fn invoke(params_pointer: u32) -> u32 {
    // Conduct method dispatch. Handle input parameters and return data.
    let ret: Option<RawBytes> = match fvm_sdk::message::method_number() {
        // Set initial value
        1 => {
            let params = params_raw(params_pointer).unwrap().1;
            let x: i64 = RawBytes::new(params).deserialize().unwrap();

            let mut state = State::load();
            state.value = x;
            state.save();

            None
        }
        // Overflow value
        2 => {
            let mut state = State::load();

            state.value = (state.value >> 1i64) * (state.value + 1);
            state.save();

            None
        }
        // Get state value
        3 => {
            let state = State::load();
            let ret = to_vec(&state.value);
            match ret {
                Ok(ret) => Some(RawBytes::new(ret)),
                Err(err) => {
                    abort(
                        ExitCode::USR_ILLEGAL_STATE.value(),
                        Some(format!("failed to serialize return value: {:?}", err).as_str()),
                    );
                }
            }
        }
        _ => abort(
            ExitCode::USR_UNHANDLED_MESSAGE.value(),
            Some("unrecognized method"),
        ),
    };

    match ret {
        None => NO_DATA_BLOCK_ID,
        Some(v) => match fvm_sdk::ipld::put_block(DAG_CBOR, v.bytes()) {
            Ok(id) => id,
            Err(err) => abort(
                ExitCode::USR_SERIALIZATION.value(),
                Some(format!("failed to store return value: {}", err).as_str()),
            ),
        },
    }
}
