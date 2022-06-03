use cid::Cid;
use fvm_ipld_encoding::de::DeserializeOwned;
use fvm_ipld_encoding::RawBytes;
use fvm_sdk::message::{params_raw, NO_DATA_BLOCK_ID};
use fvm_sdk::sself::{self_destruct, set_root};
use fvm_sdk::vm::abort;
use fvm_shared::address::Address;

fn deserialize_params<O: DeserializeOwned>(params_pointer: u32) -> O {
    let params = params_raw(params_pointer).unwrap().1;
    RawBytes::new(params).deserialize().unwrap()
}

#[no_mangle]
pub fn invoke(params_pointer: u32) -> u32 {
    // Conduct method dispatch. Handle input parameters and return data.
    match fvm_sdk::message::method_number() {
        // Abort syscall
        1 => {
            let (code, str_message): (u32, String) = deserialize_params(params_pointer);

            let mut message: Option<&str> = None;
            if str_message.len() == 0 {
                message = Some(&str_message);
            }

            abort(code, message);
        }
        // Set Root syscall
        2 => {
            let cid: Cid = deserialize_params(params_pointer);
            set_root(&cid).unwrap();
        }
        // Self destruct syscall
        3 => {
            let address: Address = deserialize_params(params_pointer);
            self_destruct(&address);
        }
        _ => abort(22, Some("unrecognized method")),
    }

    NO_DATA_BLOCK_ID
}
