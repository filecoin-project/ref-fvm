// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_sdk as sdk;
use fvm_shared::address::Address;
use fvm_shared::chainid::ChainID;
use fvm_shared::crypto::hash::SupportedHashes as SharedSupportedHashes;
use fvm_shared::crypto::signature::{Signature, SECP_SIG_LEN};
use fvm_shared::error::ErrorNumber;
use fvm_shared::sector::RegisteredSealProof;
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
    sdk::initialize();

    test_signature();
    test_expected_hash();
    test_hash_syscall();
    test_compute_unsealed_sector_cid();
    test_network_context();
    test_message_context();
    test_balance();

    #[cfg(coverage)]
    sdk::debug::store_artifact("syscall_actor.profraw", minicov::capture_coverage());
    0
}

fn test_signature() {
    // the following vectors represent a valid secp256k1 signatures for an address and plaintext message
    //
    let signature_bytes: Vec<u8> = vec![
        80, 210, 71, 248, 219, 226, 85, 142, 143, 235, 164, 155, 239, 68, 193, 23, 191, 215, 35,
        70, 25, 34, 203, 14, 116, 134, 214, 3, 91, 22, 196, 172, 105, 154, 134, 128, 228, 172, 12,
        25, 251, 166, 51, 0, 210, 45, 23, 91, 12, 18, 228, 43, 204, 157, 233, 81, 69, 3, 44, 121,
        167, 31, 168, 52, 0,
    ];
    let pub_key_bytes: Vec<u8> = vec![
        4, 223, 38, 78, 238, 254, 121, 58, 63, 120, 109, 108, 179, 105, 76, 211, 252, 223, 226, 1,
        20, 220, 212, 77, 23, 190, 224, 138, 62, 103, 27, 48, 60, 150, 151, 233, 30, 217, 137, 151,
        208, 24, 212, 117, 32, 94, 44, 118, 125, 40, 25, 31, 67, 154, 106, 97, 110, 32, 209, 62,
        194, 146, 27, 16, 114,
    ];
    let message: Vec<u8> = vec![3, 1, 4, 1, 5, 9, 2, 6, 5, 3];

    // test the happy path
    //
    let signature = Signature::new_secp256k1(signature_bytes.clone());
    let address = Address::new_secp256k1(&pub_key_bytes).unwrap();
    let res = sdk::crypto::verify_signature(&signature, &address, message.as_slice());
    assert_eq!(res, Ok(true));

    // test with invalid signature
    //
    let mut invalid_signature_bytes = signature_bytes.clone();
    invalid_signature_bytes[0] += 1;
    let invalid_signature = Signature::new_secp256k1(invalid_signature_bytes.clone());
    let res = sdk::crypto::verify_signature(&invalid_signature, &address, message.as_slice());
    assert_eq!(res, Ok(false));

    // test with invalid address
    //
    let mut invalid_pub_key_bytes = pub_key_bytes.clone();
    invalid_pub_key_bytes[0] += 1;
    let invalid_address = Address::new_secp256k1(&invalid_pub_key_bytes).unwrap();
    let res = sdk::crypto::verify_signature(&signature, &invalid_address, message.as_slice());
    assert_eq!(res, Ok(false));

    // test with invalid message
    //
    let mut invalid_message = message.clone();
    invalid_message[0] += 1;
    let res = sdk::crypto::verify_signature(&signature, &address, invalid_message.as_slice());
    assert_eq!(res, Ok(false));

    // test that calling sdk::sys::crypto::verify_signature with invalid parameters result
    // in correct error value
    //
    unsafe {
        let sig_type = signature.signature_type();
        let sig_bytes = signature.bytes();
        let signer = address.to_bytes();

        // test invalid signature type
        let res = sdk::sys::crypto::verify_signature(
            u32::MAX,
            sig_bytes.as_ptr(),
            sig_bytes.len() as u32,
            signer.as_ptr(),
            signer.len() as u32,
            message.as_ptr(),
            message.len() as u32,
        );
        assert_eq!(res, Err(ErrorNumber::IllegalArgument));

        // test invalid signature ptr
        let res = sdk::sys::crypto::verify_signature(
            sig_type as u32,
            sig_bytes.as_ptr(),
            sig_bytes.len() as u32,
            (u32::MAX) as *const u8,
            signer.len() as u32,
            message.as_ptr(),
            message.len() as u32,
        );
        assert_eq!(res, Err(ErrorNumber::IllegalArgument));
    }

    // test we can recover the public key from the signature
    //
    let hash = sdk::crypto::hash_blake2b(&message);
    let sig: [u8; SECP_SIG_LEN] = signature_bytes.try_into().unwrap();
    let res = sdk::crypto::recover_secp_public_key(&hash, &sig).unwrap();
    assert_eq!(res, pub_key_bytes.as_slice());

    // test that passing an invalid hash buffer results in IllegalArgument
    //
    unsafe {
        let res = sdk::sys::crypto::recover_secp_public_key(hash.as_ptr(), (u32::MAX) as *const u8);
        assert_eq!(res, Err(ErrorNumber::IllegalArgument));
    }
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

    // hash_owned and hash_into should return the same digest
    {
        let digest = sdk::crypto::hash_owned(SharedSupportedHashes::Blake2b512, test_bytes);
        let mut buffer = [0u8; 64];
        let len =
            sdk::crypto::hash_into(SharedSupportedHashes::Blake2b512, test_bytes, &mut buffer);
        assert_eq!(digest.len(), len);
        assert_eq!(digest.as_slice(), buffer.as_slice());
    }
}

// do funky things with hash syscall directly
fn test_hash_syscall() {
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
            u32::MAX / 2, // byte length OOB (2GB)
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
            u32::MAX / 2, // byte length OOB (2GB)
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

fn test_compute_unsealed_sector_cid() {
    // test happy path
    let pieces = Vec::new();
    sdk::crypto::compute_unsealed_sector_cid(RegisteredSealProof::StackedDRG2KiBV1, &pieces)
        .unwrap();

    // test that calling sdk::sys::crypto::compute_unsealed_sector_cid with invalid parameters
    // result in correct error value
    //
    unsafe {
        let piece: Vec<u8> = vec![];
        let mut cid: Vec<u8> = vec![];

        // should fail for invalid RegisteredSealProof
        let res = sdk::sys::crypto::compute_unsealed_sector_cid(
            999,
            piece.as_ptr(),
            piece.len() as u32,
            cid.as_mut_ptr(),
            cid.len() as u32,
        );
        assert_eq!(res, Err(ErrorNumber::IllegalArgument));
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
