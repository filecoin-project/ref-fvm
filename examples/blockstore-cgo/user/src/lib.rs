use blockstore::{cgo::CgoBlockstore, Block, Blockstore as _};
use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};

#[no_mangle]
pub extern "C" fn write_blocks(store: i32, count: i32) -> i32 {
    let bs = CgoBlockstore::new(store);
    let block = b"thing";
    for _ in 0..count {
        if bs.put(Code::Sha2_256, &Block::new(0x55, block)).is_err() {
            return 1;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn read_blocks(store: i32, count: i32) -> i32 {
    let bs = CgoBlockstore::new(store);
    let block = b"thing";
    let key = Cid::new_v1(0x55, Code::Sha2_256.digest(block));
    for _ in 0..count {
        let _ = bs.get(&key);
    }
    0
}
