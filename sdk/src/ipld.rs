use cid::Cid;

use crate::{sys, SyscallResult, MAX_CID_LEN};

/// The unit/void object.
pub const UNIT: u32 = sys::ipld::UNIT;

/// Store a block. The block will only be persisted in the state-tree if the CID is "linked in" to
/// the actor's state-tree before the end of the current invocation.
pub fn put(mh_code: u64, mh_size: u32, codec: u64, data: &[u8]) -> SyscallResult<Cid> {
    unsafe {
        let id = sys::ipld::create(codec, data.as_ptr(), data.len() as u32)?;

        let mut buf = [0u8; MAX_CID_LEN];
        let len = sys::ipld::cid(id, mh_code, mh_size, buf.as_mut_ptr(), buf.len() as u32)?;
        Ok(Cid::read_bytes(&buf[..len as usize]).expect("runtime returned an invalid CID"))
    }
}

/// Get a block. It's valid to call this on:
///
/// 1. All CIDs returned by prior calls to `get_root`...
/// 2. All CIDs returned by prior calls to `put`...
/// 3. Any children of a blocks returned by prior calls to `get`...
///
/// ...during the current invocation.
pub fn get(cid: &Cid) -> SyscallResult<Vec<u8>> {
    unsafe {
        // TODO: Check length of cid?
        let mut cid_buf = [0u8; MAX_CID_LEN];
        cid.write_bytes(&mut cid_buf[..])
            .expect("CID encoding should not fail");
        let fvm_shared::sys::out::ipld::IpldOpen { id, size, .. } =
            sys::ipld::open(cid_buf.as_mut_ptr())?;
        get_block(id, Some(size))
    }
}

/// Gets the data of the block referenced by BlockId. If the caller knows the
/// size, this function will avoid statting the block.
pub fn get_block(id: fvm_shared::sys::BlockId, size: Option<u32>) -> SyscallResult<Vec<u8>> {
    let size = match size {
        Some(size) => size,
        None => unsafe { sys::ipld::stat(id).map(|out| out.size)? },
    };
    let mut block = Vec::with_capacity(size as usize);
    unsafe {
        let bytes_read = sys::ipld::read(id, 0, block.as_mut_ptr(), size)?;
        debug_assert!(bytes_read == size, "read an unexpected number of bytes");
        block.set_len(size as usize);
    }
    Ok(block)
}

/// Writes the supplied block and returns the BlockId.
pub fn put_block(
    codec: fvm_shared::sys::Codec,
    data: &[u8],
) -> SyscallResult<fvm_shared::sys::BlockId> {
    unsafe { sys::ipld::create(codec, data.as_ptr(), data.len() as u32) }
}
