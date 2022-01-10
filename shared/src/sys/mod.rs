//! This module contains types exchanged at the syscall layer between actors
//! (usually through the SDK) and the FVM.

pub mod out;

pub type BlockId = u32;
pub type Codec = u64;

#[repr(C)]
pub struct TokenAmount {
    pub lo: u64,
    pub hi: u64,
}

impl From<TokenAmount> for crate::econ::TokenAmount {
    fn from(v: TokenAmount) -> Self {
        crate::econ::TokenAmount::from(v.hi) << 64 | crate::econ::TokenAmount::from(v.lo)
    }
}
