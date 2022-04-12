//! Syscalls for manipulating IPLD state.

/// The ID of the "unit" block (or void for C programmers).
pub const UNIT: u32 = 0;

#[doc(inline)]
pub use fvm_shared::sys::out::ipld::*;

super::fvm_syscalls! {
    module = "ipld";

    /// Opens a block from the "reachable" set, returning an ID for the block, its codec, and its
    /// size in bytes.
    ///
    /// - The reachable set is initialized to the root.
    /// - The reachable set is extended to include the direct children of loaded blocks until the
    ///   end of the invocation.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                                      |
    /// |-------------------|---------------------------------------------|
    /// | `NotFound`        | the target block isn't in the reachable set |
    /// | `IllegalArgument` | there's something wrong with the CID        |
    pub fn open(cid: *const u8) -> Result<IpldOpen>;

    /// Creates a new block, returning the block's ID. The block's children must be in the reachable
    /// set. The new block isn't added to the reachable set until the CID is computed.
    ///
    /// | Error             | Reason                                                  |
    /// |-------------------|---------------------------------------------------------|
    /// | `LimitExceeded`   | the block is too big                                    |
    /// | `NotFound`        | one of the blocks's children isn't in the reachable set |
    /// | `IllegalCodec`    | the passed codec isn't supported                        |
    /// | `Serialization`   | the passed block doesn't match the passed codec         |
    /// | `IllegalArgument` | the block isn't in memory, etc.                         |
    pub fn create(codec: u64, data: *const u8, len: u32) -> Result<u32>;

    /// Reads the block identified by `id` into `obuf`, starting at `offset`, reading _at most_
    /// `max_len` bytes.
    ///
    /// Returns the number of bytes read.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                                            |
    /// |-------------------|---------------------------------------------------|
    /// | `InvalidHandle`   | if the handle isn't known.                        |
    /// | `IllegalArgument` | if the passed buffer isn't valid, in memory, etc. |
    pub fn read(id: u32, offset: u32, obuf: *mut u8, max_len: u32) -> Result<u32>;

    /// Returns the codec and size of the specified block.
    ///
    /// # Errors
    ///
    /// | Error           | Reason                     |
    /// |-----------------|----------------------------|
    /// | `InvalidHandle` | if the handle isn't known. |
    pub fn stat(id: u32) -> Result<IpldStat>;

    // TODO: CID versions?

    /// Computes the given block's CID, writing the resulting CID into `cid`, returning the actual
    /// size of the CID.
    ///
    /// If the CID is longer than `cid_max_len`, no data is written and the actual size is returned.
    ///
    /// The returned CID is added to the reachable set.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                                            |
    /// |-------------------|---------------------------------------------------|
    /// | `InvalidHandle`   | if the handle isn't known.                        |
    /// | `IllegalCid`      | hash code and/or hash length aren't supported.    |
    /// | `IllegalArgument` | if the passed buffer isn't valid, in memory, etc. |
    pub fn cid(
        id: u32,
        hash_fun: u64,
        hash_len: u32,
        cid: *mut u8,
        cid_max_len: u32,
    ) -> Result<u32>;
}
