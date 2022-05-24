//! Syscalls for manipulating IPLD state.

/// The ID of the "unit" block (or void for C programmers).
pub const UNIT: u32 = 0;

#[doc(inline)]
pub use fvm_shared::sys::out::ipld::*;

// for documentation links
#[cfg(doc)]
use crate::sys::ErrorNumber::*;

super::fvm_syscalls! {
    module = "ipld";

    /// Opens a block from the "reachable" set, returning an ID for the block, its codec, and its
    /// size in bytes.
    ///
    /// - The reachable set is initialized to the root.
    /// - The reachable set is extended to include the direct children of loaded blocks until the
    ///   end of the invocation.
    ///
    /// # Arguments
    ///
    /// - `cid` the location of the input CID (in wasm memory).
    ///
    /// # Errors
    ///
    /// | Error               | Reason                                      |
    /// |---------------------|---------------------------------------------|
    /// | [`NotFound`]        | the target block isn't in the reachable set |
    /// | [`IllegalArgument`] | there's something wrong with the CID        |
    pub fn block_open(cid: *const u8) -> Result<IpldOpen>;

    /// Creates a new block, returning the block's ID. The block's children must be in the reachable
    /// set. The new block isn't added to the reachable set until the CID is computed.
    ///
    /// # Arguments
    ///
    /// - `codec` is the codec of the block.
    /// - `data` and `len` specify the location and length of the block data.
    ///
    /// # Errors
    ///
    /// | Error               | Reason                                                  |
    /// |---------------------|---------------------------------------------------------|
    /// | [`LimitExceeded`]   | the block is too big                                    |
    /// | [`NotFound`]        | one of the blocks's children isn't in the reachable set |
    /// | [`IllegalCodec`]    | the passed codec isn't supported                        |
    /// | [`Serialization`]   | the passed block doesn't match the passed codec         |
    /// | [`IllegalArgument`] | the block isn't in memory, etc.                         |
    pub fn block_create(codec: u64, data: *const u8, len: u32) -> Result<u32>;

    /// Reads the block identified by `id` into `obuf`, starting at `offset`, reading _at most_
    /// `max_len` bytes.
    ///
    /// Returns the difference between the length of the block and `offset + max_len`. This can be
    /// used to find the end of the block relative to the buffer the block is being read into:
    ///
    /// - A zero return value means that the block was read into the output buffer exactly.
    /// - A positive return value means that that many more bytes need to be read.
    /// - A negative return value means that the buffer should be truncated by the return value.
    ///
    /// # Arguments
    ///
    /// - `id` is ID of the block to read.
    /// - `offset` is the offset in the block to start reading.
    /// - `obuf` is the output buffer (in wasm memory) where the FVM will write the block data.
    /// - `max_len` is the maximum amount of block data to read.
    ///
    /// Passing a length/offset that exceed the length of the block will not result in an error, but
    /// will result in no data being read and a negative return value indicating where the block
    /// actually ended (relative to `offset + max_len`).
    ///
    /// # Errors
    ///
    /// | Error               | Reason                                            |
    /// |---------------------|---------------------------------------------------|
    /// | [`InvalidHandle`]   | if the handle isn't known.                        |
    /// | [`IllegalArgument`] | if the passed buffer isn't valid, in memory, etc. |
    pub fn block_read(id: u32, offset: u32, obuf: *mut u8, max_len: u32) -> Result<i32>;

    /// Returns the codec and size of the specified block.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                     |
    /// |-------------------|----------------------------|
    /// | [`InvalidHandle`] | if the handle isn't known. |
    pub fn block_stat(id: u32) -> Result<IpldStat>;

    /// Computes the given block's CID, writing the resulting CID into `cid`.
    ///
    /// The returned CID is added to the reachable set.
    ///
    /// # Arguments
    ///
    /// - `id` is ID of the block to link.
    /// - `hash_fun` is the multicodec of the hash function to use.
    /// - `hash_len` is the desired length of the hash digest.
    /// - `cid` is the output buffer (in wasm memory) where the FVM will write the resulting cid.
    /// - `cid_max_length` is the length of the output CID buffer.
    ///
    /// # Returns
    ///
    /// The length of the CID.
    ///
    /// # Errors
    ///
    /// | Error               | Reason                                            |
    /// |---------------------|---------------------------------------------------|
    /// | [`InvalidHandle`]   | if the handle isn't known.                        |
    /// | [`IllegalCid`]      | hash code and/or hash length aren't supported.    |
    /// | [`IllegalArgument`] | if the passed buffer isn't valid, in memory, etc. |
    pub fn block_link(
        id: u32,
        hash_fun: u64,
        hash_len: u32,
        cid: *mut u8,
        cid_max_len: u32,
    ) -> Result<u32>;
}
