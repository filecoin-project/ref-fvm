pub mod actor;
pub mod crypto;
pub mod error;
pub mod gas;
pub mod ipld;
pub mod message;
pub mod network;
pub mod rand;
pub mod send;
pub mod sself;
pub mod sys;
pub mod validation;
pub mod vm;

/// The maximum supported CID size. (SPEC_AUDIT)
pub const MAX_CID_LEN: usize = 100;

/// The maximum actor address length (class 2 addresses).
pub const MAX_ACTOR_ADDR_LEN: usize = 21;

// TODO: provide a custom panic handler?

#[inline]
pub(crate) fn status_code_to_bool(code: i32) -> bool {
    code == 0
}
