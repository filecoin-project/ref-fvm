use cid::Cid;
use std::ptr;

pub mod sys;

#[macro_export]
macro_rules! abort {
    () => { $crate::abort(0, None) };
    ($code:expr) => { $crate::abort($expr, None) };
    ($code:expr, $($rest:expr),+) => {
        let msg = fmt!($(rest),+);
        $crate::abort($expr, Some(&msg));
    };
}

/// Abort execution.
pub fn abort(code: u32, message: Option<&str>) -> ! {
    unsafe {
        let (message, message_len) = if let Some(m) = message {
            (m.as_ptr(), m.len())
        } else {
            (ptr::null(), 0)
        };

        sys::fvm::abort(code, message, message_len as u32);
    }
}

/* IPLD */

const MAX_CID_LEN: usize = 100;

// Transform the IPLD DAG.
pub fn transaction(f: impl FnOnce(Cid) -> Option<Cid>) {
    // TODO: Prevent calls, recursive transactions, etc.
    f(get_root()).map(set_root);
}

// Get the IPLD root.
pub fn get_root() -> Cid {
    // I really hate this CID interface. Why can't I just have bytes?
    let mut buf = [0u8; MAX_CID_LEN];
    unsafe {
        let len = crate::sys::ipld::get_root(buf.as_mut_ptr(), buf.len() as u32) as usize;
        if len > buf.len() {
            // TODO: re-try with a larger buffer?
            panic!("CID too big: {} > {}", len, buf.len())
        }
        Cid::read_bytes(&buf[..len]).expect("runtime returned an invalid CID")
    }
}

// Set the IPLD root.
pub fn set_root(cid: Cid) {
    let mut buf = [0u8; MAX_CID_LEN];
    cid.write_bytes(&mut buf[..])
        .expect("CID encoding should not fail");
    unsafe { crate::sys::ipld::set_root(buf.as_ptr()) }
}

// Store a block.
pub fn store_block(mh_code: u64, mh_size: u32, codec: u64, data: &[u8]) -> Cid {
    unsafe {
        let id = crate::sys::ipld::create(codec, data.as_ptr(), data.len() as u32);

        // I really hate this CID interface. Why can't I just have bytes?
        let mut buf = [0u8; MAX_CID_LEN];
        let len = crate::sys::ipld::cid(id, mh_code, mh_size, buf.as_mut_ptr(), buf.len() as u32)
            as usize;
        if len > buf.len() {
            // TODO: re-try with a larger buffer?
            panic!("CID too big: {} > {}", len, buf.len())
        }

        Cid::read_bytes(&buf[..len]).expect("runtime returned an invalid CID")
    }
}

// Load a block.
pub fn load_block(cid: Cid) -> Vec<u8> {
    unsafe {
        // TODO: Check length of cid?
        let mut cid_buf = [0u8; MAX_CID_LEN];
        cid.write_bytes(&mut cid_buf[..])
            .expect("CID encoding should not fail");
        let (id, _, size) = crate::sys::ipld::open(cid_buf.as_mut_ptr());
        let mut block = Vec::with_capacity(size as usize);
        let bytes_read = crate::sys::ipld::read(id, block.as_mut_ptr(), 0, size);
        assert!(bytes_read == size, "read an unexpected number of bytes");
        block.set_len(size as usize);
        block
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: this won't actually _work_ till we have an implementation of the runtime functions.
    #[test]
    pub fn test_transaction() {
        transaction(|c| {
            let data = load_block(c);
            Some(store_block(
                c.hash().code(),
                c.hash().size() as u32,
                c.codec(),
                &data,
            ))
        })
    }
}
