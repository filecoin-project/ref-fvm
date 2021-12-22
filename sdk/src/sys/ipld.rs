// Q: Charge the first time? Or every time? We have several idempotent operations here.
// A: TODO

// Q: How to allocate?
// A: Let the user do it, and reference blocks with "handles".

// Q: Ids or CIDs in set/get root?
// A: CIDs.
//   - Forces the user to explicitly call "load" if they actually want the data.
//   - Gives the user access to the CID without forcing them to recompute it.
//   - Makes the user explicitly compute the CID.

// Q: We have open, do we have close?
// A: We'd need reference counting at runtime. Not terrible, but somewhat complicated. Do we need
//    it? We probably want it in the future, but maybe not yet.
//    Idea: Use WASM "reftypes". Maybe someday.

// Q: Do we really need `stat`?
// A: No, we don't. We can punt on that if we want to.

// TODO: Implement this!
/// The ID of the "unit" block (or void for C programmers).
pub const UNIT: u32 = 0;

// TODO: new package?
#[link(wasm_import_module = "ipld")]
#[allow(improper_ctypes)]
extern "C" {
    /// Opens a block from the "reachable" set, returning an ID for the block, its codec, and its
    /// size in bytes.
    ///
    /// - The reachable set is initialized to the root.
    /// - The reachable set is extended to include the direct children of loaded blocks until the
    ///   end of the invocation.
    pub fn open(cid: *const u8) -> (u32, u32, u64, u32);

    /// Creates a new block, returning the block's ID. The block's children must be in the reachable
    /// set. The new block isn't added to the reachable set until the CID is computed.
    pub fn create(codec: u64, data: *const u8, len: u32) -> (u32, u32);

    /// Reads the identified block into obuf, starting at offset, reading _at most_ len bytes.
    /// Returns the number of bytes read.
    pub fn read(id: u32, offset: u32, obuf: *mut u8, max_len: u32) -> (u32, u32);

    /// Returns the codec and size of the specified block.
    pub fn stat(id: u32) -> (u32, u64, u32);

    // TODO: CID versions?

    /// Computes the given block's CID, returning the actual size of the CID.
    ///
    /// If the CID is longer than cid_max_len, no data is written and the actual size is returned.
    ///
    /// The returned CID is added to the reachable set.
    pub fn cid(id: u32, hash_fun: u64, hash_len: u32, cid: *mut u8, cid_max_len: u32)
        -> (u32, u32);
}
