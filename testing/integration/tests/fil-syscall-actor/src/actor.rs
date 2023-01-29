// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_sdk as sdk;
use fvm_shared::chainid::ChainID;
use fvm_shared::crypto::hash::SupportedHashes as SharedSupportedHashes;
use fvm_shared::error::ExitCode;
use multihash::derive::Multihash;
use multihash::{Blake2b256, Blake2b512, Keccak256, Ripemd160, Sha2_256};

#[derive(Clone, Copy, Debug, Eq, Multihash, PartialEq)]
#[mh(alloc_size = 64)]
// import hash functions into actor to test against output from syscall
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

#[no_mangle]
pub fn invoke(_: u32) -> u32 {
    std::panic::set_hook(Box::new(|info| {
        sdk::vm::abort(
            ExitCode::USR_ASSERTION_FAILED.value(),
            Some(&format!("{}", info)),
        )
    }));

    test_expected_hash();
    test_hash_syscall();
    test_network_context();
    test_message_context();
    test_balance();

    #[cfg(coverage)]
    sdk::debug::store_artifact("syscall_actor.profraw", minicov::capture_coverage());
    0
}

// use SDK methods to hash and compares against locally (inside the actor) hashed digest
fn test_expected_hash() {
    use multihash::MultihashDigest;
    let test_bytes = b"foo bar baz boxy";

    let blake_local = SupportedHashes::Blake2b256.digest(test_bytes);
    let blake_arr = sdk::crypto::hash_blake2b(test_bytes); // test against old SDK method since it does less unsafe things
    let blake_vec = sdk::crypto::hash_owned(SharedSupportedHashes::Blake2b256, test_bytes);

    assert_eq!(blake_arr.as_slice(), blake_vec.as_slice());
    assert_eq!(blake_local.digest(), blake_vec.as_slice());

    // macros dont work so im stuck with writing this out manually

    // blake2b512
    {
        let local_digest = SupportedHashes::Blake2b512.digest(test_bytes);
        let digest = sdk::crypto::hash_owned(SharedSupportedHashes::Blake2b512, test_bytes);

        assert_eq!(local_digest.digest(), digest.as_slice());
    }
    // sha
    {
        let local_digest = SupportedHashes::Sha2_256.digest(test_bytes);
        let digest = sdk::crypto::hash_owned(SharedSupportedHashes::Sha2_256, test_bytes);

        assert_eq!(local_digest.digest(), digest.as_slice());
    }
    // keccack
    {
        let local_digest = SupportedHashes::Keccak256.digest(test_bytes);
        let digest = sdk::crypto::hash_owned(SharedSupportedHashes::Keccak256, test_bytes);

        assert_eq!(local_digest.digest(), digest.as_slice());
    }
    // ripemd
    {
        let local_digest = SupportedHashes::Ripemd160.digest(test_bytes);
        let digest = sdk::crypto::hash_owned(SharedSupportedHashes::Ripemd160, test_bytes);

        assert_eq!(local_digest.digest(), digest.as_slice());
    }
}

// do funky things with hash syscall directly
fn test_hash_syscall() {
    use fvm_shared::error::ErrorNumber;
    use sdk::sys::crypto;

    let test_bytes = b"the quick fox jumped over the lazy dog";
    let mut buffer = [0u8; 64];

    let hasher: u64 = SharedSupportedHashes::Sha2_256 as u64;
    let known_digest = sdk::crypto::hash_owned(SharedSupportedHashes::Sha2_256, test_bytes);

    // normal case
    unsafe {
        let written = crypto::hash(
            hasher,
            test_bytes.as_ptr(),
            test_bytes.len() as u32,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        )
        .unwrap_or_else(|_| panic!("failed compute hash using {:?}", hasher));
        assert_eq!(&buffer[..written as usize], known_digest.as_slice())
    }
    // invalid hash code
    unsafe {
        let e = crypto::hash(
            0xFF,
            test_bytes.as_ptr(),
            test_bytes.len() as u32,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        )
        .expect_err("Expected err from invalid code, got written bytes");
        assert_eq!(e, ErrorNumber::IllegalArgument)
    }
    // data pointer OOB
    unsafe {
        let e = crypto::hash(
            hasher,
            (u32::MAX) as *const u8, // pointer OOB
            test_bytes.len() as u32,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        )
        .expect_err("Expected err, got written bytes");
        assert_eq!(e, ErrorNumber::IllegalArgument)
    }
    // data length OOB
    unsafe {
        let e = crypto::hash(
            hasher,
            test_bytes.as_ptr(),
            (u32::MAX / 2) as u32, // byte length OOB (2GB)
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        )
        .expect_err("Expected err, got written bytes");
        assert_eq!(e, ErrorNumber::IllegalArgument)
    }
    // digest buffer pointer OOB
    unsafe {
        let e = crypto::hash(
            hasher,
            test_bytes.as_ptr(),
            test_bytes.len() as u32,
            (u32::MAX) as *mut u8, // pointer OOB
            buffer.len() as u32,
        )
        .expect_err("Expected err, got written bytes");
        assert_eq!(e, ErrorNumber::IllegalArgument)
    }
    // digest length out of memory
    unsafe {
        let e = crypto::hash(
            hasher,
            test_bytes.as_ptr(),
            test_bytes.len() as u32,
            buffer.as_mut_ptr(),
            (u32::MAX / 2) as u32, // byte length OOB (2GB)
        )
        .expect_err("Expected err, got written bytes");
        assert_eq!(e, ErrorNumber::IllegalArgument)
    }
    // write bytes to the same buffer read from. (overlapping buffers is OK)
    unsafe {
        let len = test_bytes.len();
        // fill with "garbage"
        buffer.fill(0x69);
        buffer[..len].copy_from_slice(test_bytes);

        let written = crypto::hash(
            hasher,
            // read from buffer...
            buffer.as_ptr(),
            len as u32,
            // and write to the same one
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        )
        .expect("Overlapping buffers should be allowed");
        assert_eq!(&buffer[..written as usize], known_digest.as_slice())
    }
}

fn test_network_context() {
    use fvm_shared::econ::TokenAmount;
    use fvm_shared::version::NetworkVersion;
    assert_eq!(sdk::network::chain_id(), ChainID::from(1)); // hehe we are ETH now
    assert_eq!(sdk::network::curr_epoch(), 0);
    assert_eq!(sdk::network::version(), NetworkVersion::V18);
    assert_eq!(sdk::network::tipset_timestamp(), 0);
    assert_eq!(sdk::network::base_fee(), TokenAmount::from_atto(100));
}

fn test_message_context() {
    assert_eq!(sdk::message::nonce(), 100);
    assert_eq!(sdk::message::origin(), 100);
    assert_eq!(sdk::message::caller(), 100);
    assert_eq!(sdk::message::receiver(), 10000);
    assert_eq!(sdk::message::method_number(), 1);
    assert!(sdk::message::value_received().is_zero());
    assert!(sdk::message::gas_premium().is_zero());
}

fn test_balance() {
    // Getting the balance of a non-existent actor should return None.
    assert_eq!(sdk::actor::balance_of(9191919), None);

    // Our balance should match.
    assert_eq!(
        sdk::actor::balance_of(sdk::message::receiver()),
        Some(sdk::sself::current_balance())
    );
}
