//! Syscalls for creating and resolving actors.

#[doc(inline)]
pub use fvm_shared::sys::out::actor::*;

super::fvm_syscalls! {
    module = "actor";

    /// Resolves the ID address of an actor.
    ///
    /// # Errors
    ///
    /// | Error             | Reason                                                    |
    /// |-------------------|-----------------------------------------------------------|
    /// | `NotFound`        | target actor doesn't exist                                |
    /// | `IllegalArgument` | if the passed address buffer isn't valid, in memory, etc. |
    pub fn resolve_address(
        addr_off: *const u8,
        addr_len: u32,
    ) -> Result<ResolveAddress>;

    /// Gets the CodeCID of an actor by address.
    ///
    /// Returns the
    ///
    /// # Errors
    ///
    /// | Error             | Reason                                                    |
    /// |-------------------|-----------------------------------------------------------|
    /// | `NotFound`        | target actor doesn't exist                                |
    /// | `IllegalArgument` | if the passed address buffer isn't valid, in memory, etc. |
    pub fn get_actor_code_cid(
        addr_off: *const u8,
        addr_len: u32,
        obuf_off: *mut u8,
        obuf_len: u32,
    ) -> Result<i32>;

    /// Determines whether the specified CodeCID belongs to that of a builtin
    /// actor and which. Returns 0 if unrecognized. Can only fail due to
    /// internal errors.
    pub fn resolve_builtin_actor_type(cid_off: *const u8) -> Result<i32>;

     /// Returns the CodeCID for the given built-in actor type. Aborts with exit
     /// code IllegalArgument if the supplied type is invalid. Returns the
     /// length of the written CID written to the output buffer. Can only
     /// return a failure due to internal errors.
    pub fn get_code_cid_for_type(typ: i32, obuf_off: *mut u8, obuf_len: u32) -> Result<i32>;

    /// Generates a new actor address for an actor deployed
    /// by the calling actor.
    ///
    /// **Privledged:** May only be called by the init actor.
    #[doc(hidden)]
    pub fn new_actor_address(obuf_off: *mut u8, obuf_len: u32) -> Result<u32>;

    /// Creates a new actor of the specified type in the state tree, under
    /// the provided address.
    ///
    /// **Privledged:** May only be called by the init actor.
    #[doc(hidden)]
    pub fn create_actor(actor_id: u64, typ_off: *const u8) -> Result<()>;
}
