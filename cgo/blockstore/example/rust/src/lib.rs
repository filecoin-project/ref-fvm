use cgobs::Blockstore;
use cid::multihash::{Code, MultihashDigest};
use cid::Cid;

#[no_mangle]
pub extern "C" fn write_a_block(store: i32) -> i32 {
    let bs = unsafe { Blockstore::new(store) };
    let block = b"thing";
    let key = Cid::new_v1(0x55, Code::Sha2_256.digest(block));
    match bs.put(&key, block) {
        Ok(_) => 0,
        Err(_) => 1,
    }
}
