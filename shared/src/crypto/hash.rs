use multihash::derive::Multihash;
use multihash::{Sha2_256, Blake2b256, Keccak256};

#[derive(Clone, Copy, Debug, Eq, Multihash, PartialEq)]
#[mh(alloc_size = 64)]
pub enum FvmHashCode {
    #[mh(code = 0x12, hasher = Sha2_256)]
    Sha2_256,
    #[mh(code = 0xb220, hasher = Blake2b256)]
    Blake2b256,
    #[mh(code = 0x1b, hasher = Keccak256)]
    Keccak256,
    // TODO ripemd
}