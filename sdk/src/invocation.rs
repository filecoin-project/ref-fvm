// TODO: this API kind of sucks. It's a good start, but we need easy decoding.
// It's also in the wrong place.

/// Returns the message codec and parameters.
pub fn params() -> (u64, Vec<u8>) {
    // Defined by the system.
    const PARAMS_ID: u32 = 0;

    // I really hate this CID interface. Why can't I just have bytes?
    unsafe {
        let (codec, size) = crate::sys::ipld::stat(PARAMS_ID);

        let mut block = Vec::with_capacity(size as usize);
        let bytes_read = crate::sys::ipld::read(PARAMS_ID, 0, block.as_mut_ptr(), size);
        debug_assert!(bytes_read == size, "read an unexpected number of bytes");
        block.set_len(size as usize);
        (codec, block)
    }
}

// Making these separate functions may make upgrading/refactoring easier. We can revisit in the future.

// TODO: enum with a bigint variant?
pub type TokenAmount = u128;

// TODO: dedup with actors.
pub type ActorID = u64;
pub type MethodNum = u64;

/// Returns the message's method number.
#[inline(always)]
pub fn method() -> MethodNum {
    crate::sys::METADATA.method
}

/// Returns the ID address of the caller.
#[inline(always)]
pub fn caller() -> ActorID {
    // I really hate this CID interface. Why can't I just have bytes?
    crate::sys::METADATA.caller
}

// TODO: bad name.
/// Returns the ID address of the actor.
#[inline(always)]
pub fn receiver() -> ActorID {
    // I really hate this CID interface. Why can't I just have bytes?
    crate::sys::METADATA.receiver
}

/// Returns the value received from the caller in AttoFIL.
#[inline(always)]
pub fn value_received() -> TokenAmount {
    crate::sys::METADATA.value_received.into()
}
