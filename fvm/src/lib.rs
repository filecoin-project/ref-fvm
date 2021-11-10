use blockstore::Blockstore;
use wasmer::{Function, ImportObject, Store};

struct Env<B> {
    bs: B,
}

impl<B> Env where B: Blockstore {}

// TODO: this really needs an env. BUT, in order to get that to work, we need to handle some lifetime problems...
fn get_root(_cid: i32, _cid_max_len: i32) -> i32 {
    println!("root requested");
    0
}

pub fn environment(store: &Store) -> ImportObject {
    let get_root_function = Function::new_native(store, get_root);
    wasmer::imports! {
        "ipld" => {
            "get_root" => get_root_function,
        }
    }
}
