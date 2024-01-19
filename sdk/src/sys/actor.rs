// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
//! Syscalls for creating and resolving actors.

#[doc(inline)]
pub use fvm_shared::sys::out::send::*;

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

    /// Looks up the "delegated" (f4) address of the target actor (if any).
    ///
    /// # Arguments
    ///
    /// `addr_buf_off` and `addr_buf_len` specify the location and length of the output buffer in
    /// which to store the address.
    ///
    /// # Returns
    ///
    /// The length of the address written to the output buffer, or 0 if the target actor has no
    /// delegated (f4) address.
    ///
    /// # Errors
    ///
    /// | Error               | Reason                                                           |
    /// |---------------------|------------------------------------------------------------------|
    /// | [`NotFound`]        | if the target actor does not exist                               |
    /// | [`BufferTooSmall`]  | if the output buffer isn't large enough to fit the address       |
    /// | [`IllegalArgument`] | if the output buffer isn't valid, in memory, etc.                |
    pub fn lookup_delegated_address(
        actor_id: u64,
        addr_buf_off: *mut u8,
        addr_buf_len: u32,
    ) -> Result<u32>;


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

    /// Generates a new actor address for an actor deployed by the calling actor.
    ///
    /// **Privileged:** May only be called by the init actor.
    #[doc(hidden)]
    pub fn next_actor_address(obuf_off: *mut u8, obuf_len: u32) -> Result<u32>;

    /// Creates a new actor in the state-tree with the specified actor ID, recording the specified
    /// "delegated" address in the actor root if non-empty, and returning a new stable address.
    ///
    /// **Privileged:** May only be called by the init actor.
    #[doc(hidden)]
    pub fn create_actor(
        actor_id: u64,
        typ_off: *const u8,
        delegated_addr_off: *const u8,
        delegated_addr_len: u32,
    ) -> Result<()>;


    /// Atomically transition to the new actor code. On success, this syscall does not return to the
    /// current actor. Instead, the target actor "replaces" the invocation.
    ///
    /// # Parameters
    ///
    /// - `new_code_cid_off` is the offset (in wasm memory) of the code CID to upgrade _to_.
    /// - `params` is the IPLD block handle passed to the new code's `upgrade` wasm endpoint.
    ///
    /// # Returns
    ///
    /// On successful upgrade, this syscall will not return. Instead, the current invocation will
    /// "complete" and the return value will be the block returned by the new code's `upgrade` endpoint.
    ///
    /// If the new code rejects the upgrade (aborts) or performs an illegal operation, this syscall will
    /// return the exit code plus the error returned by the upgrade endpoint.
    ///
    /// Finally, the syscall will return an error if it fails to call the upgrade endpoint entirely.
    ///
    /// # Errors
    ///
    /// | Error                 | Reason                                                          |
    /// |-----------------------|-----------------------------------------------------------------|
    /// | [`NotFound`]          | no code with the specified CID has been deployed.               |
    /// | [`IllegalOperation`]  | the actor has been deleted.                                     |
    /// | [`InvalidHandle`]     | parameters block not found.                                     |
    /// | [`LimitExceeded`]     | recursion limit reached.                                        |
    /// | [`IllegalArgument`]   | invalid code cid buffer.                                        |
    /// | [`Forbidden`]         | the actor is not allowed to upgrade (e.g., due to re-entrency). |
    /// | [`ReadOnly`]          | the actor is executing in read-only mode.                       |
    #[cfg(feature = "upgrade-actor")]
    pub fn upgrade_actor(
        new_code_cid_off: *const u8,
        params: u32,
    ) -> Result<Send>;

    /// Installs and ensures actor code is valid and loaded.
    /// **Privileged:** May only be called by the init actor.
    #[cfg(feature = "m2-native")]
    pub fn install_actor(cid_off: *const u8) -> Result<()>;

    /// Gets the balance of the specified actor.
    ///
    /// # Arguments
    ///
    /// - `actor_id` is the ID of the target actor.
    ///
    /// # Errors
    ///
    /// | Error                | Reason                                         |
    /// |----------------------|------------------------------------------------|
    /// | [`NotFound`]         | the target actor does not exist                |
    pub fn balance_of(
        actor_id: u64
    )  -> Result<super::TokenAmount>;
}
