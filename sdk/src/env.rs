use crate::sys;

pub fn timestamp() -> u64 {
    unsafe { sys::env::timestamp() }.expect("failed to get timestamp")
}

pub fn blockhash(block: u8) -> [u8; 32] {
    let ret = [0u8; 32];
    unsafe { sys::env::blockhash(block, ret.as_ptr(), ret.len() as u32) }
        .expect("failed to get blockhash");
    ret
}
