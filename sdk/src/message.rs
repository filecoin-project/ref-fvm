// TODO: this API kind of sucks. It's a good start, but we need easy decoding.
// It's also in the wrong place.

/// Returns the message codec and parameters.
pub fn params() -> (u64, Vec<u8>) {
    // I really hate this CID interface. Why can't I just have bytes?
    unsafe {
        let params_id = crate::sys::fvm::message_params(); // TODO: we _could_ just define this to be 0 and save a call.
        let (codec, size) = crate::sys::ipld::stat(params_id);

        let mut block = Vec::with_capacity(size as usize);
        let bytes_read = crate::sys::ipld::read(params_id, 0, block.as_mut_ptr(), size);
        debug_assert!(bytes_read == size, "read an unexpected number of bytes");
        block.set_len(size as usize);
        (codec, block)
    }
}

/// Returns the ID address of the caller.
pub fn caller() -> u64 {
    // I really hate this CID interface. Why can't I just have bytes?
    unsafe { crate::sys::fvm::message_caller() }
}

// TODO: bad name.
/// Returns the ID address of the actor.
pub fn receiver() -> u64 {
    // I really hate this CID interface. Why can't I just have bytes?
    unsafe { crate::sys::fvm::message_receiver() }
}
