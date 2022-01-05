#[link(wasm_import_module = "vm")]
extern "C" {
    /* Control */

    /// Abort execution with the given code and message. The code is recorded in the receipt, the
    /// message is for debugging only.
    pub fn abort(code: u32, message: *const u8, message_len: u32) -> !;

    // Revert any state-changes and return the IPLD block referenced by the passed block ID.
    // TODO: There's no way to currently revert _and_ succeed/return a value. But that seems useful.
    // pub fn revert(id: u32) -> !;

    // Commit any state-changes and return the IPLD block referenced by the passed block ID.
    // TODO: The current FVM doesn't currently have a concept of "short-circuit" finishing. Instead,
    // we just return the final value.
    // pub fn finish(id: u32) -> !;

    /* TODO Syscalls */

    // Ignored for now. These should all be pre-compiles, not syscalls.
    // Except verify consensus fault... Which needs to look back in history. Can we just kill that?
}
