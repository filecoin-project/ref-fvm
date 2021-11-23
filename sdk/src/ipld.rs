use cid::Cid;

/// The maximum supported CID size. (SPEC_AUDIT)
pub const MAX_CID_LEN: usize = 100;

/// The unit/void object.
pub const UNIT: u32 = crate::sys::ipld::UNIT;

/// Get the IPLD root CID.
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

/// Set the actor's state-tree root. The CID must be in the "reachable" set.
pub fn set_root(cid: &Cid) {
    let mut buf = [0u8; MAX_CID_LEN];
    cid.write_bytes(&mut buf[..])
        .expect("CID encoding should not fail");
    unsafe { crate::sys::ipld::set_root(buf.as_ptr()) }
}

/// Store a block. The block will only be persisted in the state-tree if the CID is "linked in" to
/// the actor's state-tree before the end of the current invocation.
pub fn put(mh_code: u64, mh_size: u32, codec: u64, data: &[u8]) -> Cid {
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

/// Get a block. It's valid to call this on:
///
/// 1. All CIDs returned by prior calls to `get_root`...
/// 2. All CIDs returned by prior calls to `put`...
/// 3. Any children of a blocks returned by prior calls to `get`...
///
/// ...during the current invocation.
pub fn get(cid: &Cid) -> Vec<u8> {
    unsafe {
        // TODO: Check length of cid?
        let mut cid_buf = [0u8; MAX_CID_LEN];
        cid.write_bytes(&mut cid_buf[..])
            .expect("CID encoding should not fail");
        let (id, _, size) = crate::sys::ipld::open(cid_buf.as_mut_ptr());
        let mut block = Vec::with_capacity(size as usize);
        let bytes_read = crate::sys::ipld::read(id, 0, block.as_mut_ptr(), size);
        debug_assert!(bytes_read == size, "read an unexpected number of bytes");
        block.set_len(size as usize);
        block
    }
}

// Transform the IPLD DAG.
pub fn transaction(f: impl FnOnce(Cid) -> Option<Cid>) {
    // TODO: Prevent calls, recursive transactions, etc.
    f(get_root()).as_ref().map(set_root);
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: this won't actually _work_ till we have an implementation of the runtime functions.
    #[test]
    pub fn test_transaction() {
        transaction(|c| {
            let data = get(&c);
            Some(put(
                c.hash().code(),
                c.hash().size() as u32,
                c.codec(),
                &data,
            ))
        })
    }
}
