use cid::Cid;
use fvm_shared::error::ErrorNumber;
use fvm_shared::MAX_CID_LEN;

use crate::sys;

pub fn tipset_timestamp() -> u64 {
    unsafe { sys::chain::tipset_timestamp() }.expect("failed to get timestamp")
}

pub fn tipset_cid(epoch: i64) -> Option<Cid> {
    let mut buf = [0u8; MAX_CID_LEN];

    unsafe {
        match sys::chain::tipset_cid(epoch, buf.as_mut_ptr(), MAX_CID_LEN as u32) {
            Ok(len) => Some(Cid::read_bytes(&buf[..len as usize]).expect("invalid cid")),
            Err(ErrorNumber::NotFound) => None,
            Err(other) => panic!("unexpected cid resolution failure: {}", other),
        }
    }
}
