use crate::error::{IntoSyscallResult, SyscallResult};
use crate::{sself, sys, MAX_CID_LEN};
use cid::Cid;
use fvm_shared::encoding::DAG_CBOR;

/// The unit/void object.
pub const UNIT: u32 = sys::ipld::UNIT;

pub type BlockId = u32;
pub type Codec = u64;

/// Store a block. The block will only be persisted in the state-tree if the CID is "linked in" to
/// the actor's state-tree before the end of the current invocation.
pub fn put(mh_code: u64, mh_size: u32, codec: u64, data: &[u8]) -> SyscallResult<Cid> {
    unsafe {
        let id =
            sys::ipld::create(codec, data.as_ptr(), data.len() as u32).into_syscall_result()?;

        // I really hate this CID interface. Why can't I just have bytes?
        let mut buf = [0u8; MAX_CID_LEN];
        let len = sys::ipld::cid(id, mh_code, mh_size, buf.as_mut_ptr(), buf.len() as u32)
            .into_syscall_result()? as usize;
        if len > buf.len() {
            // TODO: re-try with a larger buffer?
            panic!("CID too big: {} > {}", len, buf.len())
        }

        Ok(Cid::read_bytes(&buf[..len]).expect("runtime returned an invalid CID"))
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
        let (id, _, size) = sys::ipld::open(cid_buf.as_mut_ptr()).into_syscall_result()?;
        get_block(id, Some(size))
    }
}

/// Gets the data of the block referenced by BlockId. If the caller knows the
/// size, this function will avoid statting the block.
pub fn get_block(id: BlockId, size: Option<u32>) -> SyscallResult<Vec<u8>> {
    let size = match size {
        Some(size) => size,
        None => unsafe {
            sys::ipld::stat(id)
                .into_syscall_result()
                .map(|(_, size)| size)?
        },
    };
    let mut block = Vec::with_capacity(size as usize);
    unsafe {
        let bytes_read = sys::ipld::read(id, 0, block.as_mut_ptr(), size).into_syscall_result()?;
        debug_assert!(bytes_read == size, "read an unexpected number of bytes");
        block.set_len(size as usize);
    }
    Ok(block)
}

/// Writes the supplied block and returns the BlockId.
pub fn put_block(codec: Codec, data: &[u8]) -> SyscallResult<BlockId> {
    unsafe { sys::ipld::create(codec, data.as_ptr(), data.len() as u32).into_syscall_result() }
}

// Transform the IPLD DAG.
pub fn transaction(f: impl FnOnce(Cid) -> Option<Cid>) -> SyscallResult<()> {
    // TODO: Prevent calls, recursive transactions, etc.
    f(sself::get_root()?).as_ref().map(sself::set_root);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: this won't actually _work_ till we have an implementation of the runtime functions.
    #[test]
    pub fn test_transaction() {
        transaction(|c| {
            let data = get(&c).unwrap();
            Some(put(c.hash().code(), c.hash().size() as u32, c.codec(), &data).unwrap())
        })
        .unwrap()
    }
}
