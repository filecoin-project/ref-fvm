// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::multihash::Multihash;
use cid::Cid;
use fvm_shared::error::ErrorNumber;
use fvm_shared::MAX_CID_LEN;

use crate::{sys, SyscallResult};

/// The unit/void object.
pub const UNIT: u32 = sys::ipld::UNIT;

/// Store a block. The block will only be persisted in the state-tree if the CID is "linked in" to
/// the actor's state-tree before the end of the current invocation.
pub fn put(mh_code: u64, mh_size: u32, codec: u64, data: &[u8]) -> SyscallResult<Cid> {
    // Short-circuit for identity hashes. There's nothing to "put" as the block will be embedded in
    // the CID directly.
    if mh_code == fvm_shared::IDENTITY_HASH {
        if mh_size as usize != data.len() {
            return Err(ErrorNumber::IllegalCid);
        }
        // NOTE: We don't check the codec here intentionally. The user will get an error if/when
        // they actually try to put a block _containing_ this identity-hashed CID.
        //
        // We do this for forwards-compatibility. If we checked the codec, old contracts may end up
        // rejecting some IPLD codec we allow in the future (even if that data _should_ be opaque to
        // the contract).
        return Ok(Cid::new_v1(
            codec,
            Multihash::wrap(mh_code, data).map_err(|_| ErrorNumber::IllegalCid)?,
        ));
    }

    unsafe {
        let id = sys::ipld::block_create(codec, data.as_ptr(), data.len() as u32)?;

        let mut buf = [0u8; MAX_CID_LEN];
        let len = sys::ipld::block_link(id, mh_code, mh_size, buf.as_mut_ptr(), buf.len() as u32)?;
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
    // Short-circuit for identity hashes. There's nothing to "get" as the block is inlined in teh
    // hash itself.
    if cid.hash().code() == fvm_shared::IDENTITY_HASH {
        return Ok(cid.hash().digest().into());
    }

    unsafe {
        let mut cid_buf = [0u8; MAX_CID_LEN];
        cid.write_bytes(&mut cid_buf[..])
            .expect("CID encoding should not fail");
        let fvm_shared::sys::out::ipld::IpldOpen { id, size, .. } =
            sys::ipld::block_open(cid_buf.as_mut_ptr())?;
        let mut block = Vec::with_capacity(size as usize);
        let remaining = sys::ipld::block_read(id, 0, block.as_mut_ptr(), size)?;
        assert_eq!(remaining, 0, "expected to read the block exactly");
        block.set_len(size as usize);
        Ok(block)
    }
}

/// Gets the data of the block referenced by BlockId. If the caller knows the size, this function
/// will read the block in a single syscall. Otherwise, any block over 1KiB will take two syscalls.
pub fn get_block(id: fvm_shared::sys::BlockId, size_hint: Option<u32>) -> SyscallResult<Vec<u8>> {
    // Check for the "empty" block first.
    if id == UNIT {
        return Ok(Vec::new());
    }

    let mut buf = Vec::with_capacity(size_hint.unwrap_or(1024) as usize);
    unsafe {
        let mut remaining = sys::ipld::block_read(id, 0, buf.as_mut_ptr(), buf.capacity() as u32)?;
        if remaining > 0 {
            buf.set_len(buf.capacity());
            buf.reserve_exact(remaining as usize);
            remaining = sys::ipld::block_read(
                id,
                buf.len() as u32,
                buf.as_mut_ptr_range().end,
                (buf.capacity() - buf.len()) as u32,
            )?;
            assert!(remaining <= 0, "should have read whole block");
        }
        let size = (buf.capacity() as i64) + (remaining as i64);
        assert!(size >= 0, "size can't be negative");
        assert!(
            size <= buf.capacity() as i64,
            "size shouldn't exceed capacity"
        );
        buf.set_len(size as usize);
    }
    Ok(buf)
}

/// Writes the supplied block and returns the BlockId.
pub fn put_block(
    codec: fvm_shared::sys::Codec,
    data: &[u8],
) -> SyscallResult<fvm_shared::sys::BlockId> {
    unsafe { sys::ipld::block_create(codec, data.as_ptr(), data.len() as u32) }
}
