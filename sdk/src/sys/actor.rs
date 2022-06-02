//! Syscalls for creating and resolving actors.

// for documentation links
#[cfg(doc)]
use crate::sys::ErrorNumber::*;

super::fvm_syscalls! {
    module = "actor";

    /// Resolves the ID address of an actor.
    ///
    /// # Arguments
    ///
    /// `addr_off` and `addr_len` specify the location and length of an address to be resolved.
    ///
    /// # Errors
    ///
    /// | Error               | Reason                                                    |
    /// |---------------------|-----------------------------------------------------------|
    /// | [`NotFound`]        | if the target actor does not exist                        |
    /// | [`IllegalArgument`] | if the passed address buffer isn't valid, in memory, etc. |
    pub fn resolve_address(
        addr_off: *const u8,
        addr_len: u32,
    ) -> Result<u64>;

    /// Gets the CodeCID of an actor by address.
    ///
    /// # Arguments
    ///
    /// - `actor_id` is the resolved ID of the target actor.
    /// - `obuf_off` and `obuf_len` specify the location and length of a byte buffer into which the
    ///   FVM will write the actor's code CID, if the actor is found.
    ///
    /// # Returns
    ///
    /// The length of the CID.
    ///
    /// # Errors
    ///
    /// | Error               | Reason                                                    |
    /// |---------------------|-----------------------------------------------------------|
    /// | [`NotFound`]        | if the target actor does not exist                        |
    /// | [`BufferTooSmall`]  | if the output buffer isn't large enough to fit the CID    |
    /// | [`IllegalArgument`] | if the passed address buffer isn't valid, in memory, etc. |
    pub fn get_actor_code_cid(
        actor_id: u64,
        obuf_off: *mut u8,
        obuf_len: u32,
    ) -> Result<u32>;

    /// Returns the builtin-actor type ID for the given CodeCID, or 0 if the CodeCID is not a
    /// builtin actor.
    ///
    /// # Arguments
    ///
    /// - `cid_off` specifies the cid to be resolved.
    ///
    /// # Errors
    ///
    /// | Error               | Reason                                                    |
    /// |---------------------|-----------------------------------------------------------|
    /// | [`IllegalArgument`] | if the passed CID isn't valid                             |
    pub fn get_builtin_actor_type(cid_off: *const u8) -> Result<i32>;

    /// Returns the CodeCID for the given built-in actor type.
    ///
    /// # Arguments
    ///
    /// - `typ` specifies the builtin-actor [`Type`] to lookup.
    /// - `obuf_off` and `obuf_len` specify the location and length of a byte buffer into which the
    ///   FVM will write the s code CID.
    ///
    /// # Returns
    ///
    /// The length of the code CID.
    ///
    /// # Errors
    ///
    /// | Error               | Reason                                                          |
    /// |---------------------|-----------------------------------------------------------------|
    /// | [`IllegalArgument`] | if the type is invalid, or the outupt buffer isn't large enough |
    pub fn get_code_cid_for_type(typ: i32, obuf_off: *mut u8, obuf_len: u32) -> Result<u32>;

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
