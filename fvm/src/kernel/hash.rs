// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use multihash::derive::Multihash;
use multihash::{Blake2b256, Blake2b512, Keccak256, Ripemd160, Sha2_256};

#[derive(Clone, Copy, Debug, Eq, Multihash, PartialEq, Hash)]
#[mh(alloc_size = 64)]
/// Codes and hashers supported by FVM.
/// You _can_ use this hash directly inside of your actor,
/// but it will very likely be more performant with the `hash` syscall
pub enum SupportedHashes {
    #[mh(code = 0x12, hasher = Sha2_256)]
    Sha2_256,
    #[mh(code = 0xb220, hasher = Blake2b256)]
    Blake2b256,
    #[mh(code = 0xb240, hasher = Blake2b512)]
    Blake2b512,
    #[mh(code = 0x1b, hasher = Keccak256)]
    Keccak256,
    #[mh(code = 0x1053, hasher = Ripemd160)]
    Ripemd160,
}
