// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use fvm_sdk as sdk;
use fvm_sdk::sys::network::{context, NetworkContext};
use fvm_shared::address::Address;
use fvm_shared::chainid::ChainID;
use fvm_shared::crypto::hash::SupportedHashes as SharedSupportedHashes;
use fvm_shared::crypto::signature::{
    Signature, BLS_DIGEST_LEN, BLS_PUB_LEN, BLS_SIG_LEN, SECP_SIG_LEN, SECP_SIG_MESSAGE_HASH_SIZE,
};
use fvm_shared::error::ErrorNumber;
use fvm_shared::sector::RegisteredSealProof;
use multihash::derive::Multihash;
use multihash::{Blake2b256, Blake2b512, Keccak256, Ripemd160, Sha2_256};
use std::ptr;

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

    test_secp_signature();
    test_bls_aggregate();
    test_expected_hash();
    test_hash_syscall();
    test_compute_unsealed_sector_cid();
    test_network_context();
    test_message_context();
    test_balance();
    test_unaligned();
    test_upgrade();

    #[cfg(coverage)]
    sdk::debug::store_artifact("syscall_actor.profraw", minicov::capture_coverage());
    0
}

fn test_secp_signature() {
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
    let digest = sdk::crypto::hash_blake2b(&message);

    // test the happy path
    //
    let signature = Signature::new_secp256k1(signature_bytes.clone());
    let address = Address::new_secp256k1(&pub_key_bytes).unwrap();
    let res = sdk::crypto::verify_signature(&signature, &[address], &[&digest]);
    assert_eq!(res, Ok(true));

    // test with invalid signature
    //
    let mut invalid_signature_bytes = signature_bytes.clone();
    invalid_signature_bytes[0] += 1;
    let invalid_signature = Signature::new_secp256k1(invalid_signature_bytes.clone());
    let res = sdk::crypto::verify_signature(&invalid_signature, &[address], &[&digest]);
    assert_eq!(res, Ok(false));

    // test with invalid address
    //
    let mut invalid_pub_key_bytes = pub_key_bytes.clone();
    invalid_pub_key_bytes[0] += 1;
    let invalid_address = Address::new_secp256k1(&invalid_pub_key_bytes).unwrap();
    let res = sdk::crypto::verify_signature(&signature, &[invalid_address], &[&digest]);
    assert_eq!(res, Ok(false));

    // test with invalid digest
    //
    let mut invalid_digest = digest;
    invalid_digest[0] += 1;
    let res = sdk::crypto::verify_signature(&signature, &[address], &[&invalid_digest]);
    assert_eq!(res, Ok(false));

    // test we can recover the public key from the signature
    //
    let digest: &[u8; SECP_SIG_MESSAGE_HASH_SIZE] = digest.as_slice().try_into().unwrap();
    let sig: &[u8; SECP_SIG_LEN] = signature_bytes.as_slice().try_into().unwrap();
    let res = sdk::crypto::recover_secp_public_key(digest, sig).unwrap();
    assert_eq!(res, pub_key_bytes.as_slice());

    // test that passing an invalid hash buffer results in IllegalArgument
    //
    unsafe {
        let res =
            sdk::sys::crypto::recover_secp_public_key(digest.as_ptr(), (u32::MAX) as *const u8);
        assert_eq!(res, Err(ErrorNumber::IllegalArgument));
    }
}

fn test_bls_aggregate() {
    let pub_keys: [[u8; BLS_PUB_LEN]; 3] = [
        [
            177, 126, 78, 182, 93, 122, 198, 81, 5, 240, 226, 238, 241, 247, 37, 183, 171, 231,
            237, 71, 215, 84, 120, 150, 238, 23, 45, 109, 96, 19, 169, 23, 115, 147, 70, 45, 36,
            87, 177, 103, 43, 231, 60, 58, 127, 63, 232, 225,
        ],
        [
            129, 103, 1, 32, 207, 243, 63, 21, 153, 244, 175, 228, 198, 117, 233, 143, 194, 93, 2,
            243, 0, 76, 118, 90, 253, 135, 217, 156, 253, 206, 122, 235, 193, 127, 106, 30, 20,
            236, 34, 250, 33, 137, 153, 105, 188, 93, 23, 120,
        ],
        [
            185, 119, 106, 3, 95, 233, 17, 93, 47, 218, 127, 209, 128, 81, 141, 173, 58, 128, 118,
            65, 28, 115, 204, 155, 166, 63, 44, 14, 155, 166, 46, 29, 219, 18, 74, 105, 64, 99, 91,
            18, 197, 99, 30, 190, 173, 166, 184, 37,
        ],
    ];

    let digests: [[u8; BLS_DIGEST_LEN]; 3] = [
        [
            132, 82, 107, 94, 117, 95, 20, 70, 162, 244, 52, 179, 230, 89, 249, 67, 73, 78, 87,
            226, 38, 245, 100, 202, 82, 71, 23, 200, 52, 77, 119, 142, 88, 10, 205, 242, 168, 220,
            124, 205, 106, 17, 42, 70, 2, 101, 152, 48, 15, 25, 137, 194, 234, 252, 168, 123, 104,
            115, 245, 134, 52, 82, 98, 112, 175, 60, 187, 114, 41, 174, 236, 80, 81, 228, 213, 190,
            255, 219, 192, 89, 45, 107, 57, 106, 204, 173, 182, 193, 253, 166, 111, 153, 49, 157,
            241, 6,
        ],
        [
            143, 83, 122, 171, 144, 138, 124, 244, 188, 64, 75, 200, 113, 60, 60, 182, 192, 214,
            12, 12, 63, 206, 4, 124, 2, 108, 161, 168, 153, 189, 219, 8, 62, 210, 53, 85, 237, 69,
            53, 245, 205, 202, 165, 227, 14, 251, 125, 189, 12, 238, 220, 232, 99, 108, 163, 170,
            237, 54, 156, 235, 93, 234, 120, 69, 251, 2, 214, 176, 180, 57, 176, 247, 147, 4, 130,
            50, 203, 205, 99, 208, 158, 104, 82, 2, 29, 145, 68, 153, 158, 62, 77, 46, 99, 168,
            218, 147,
        ],
        [
            183, 110, 18, 193, 253, 70, 141, 158, 111, 99, 127, 135, 254, 94, 113, 208, 219, 94,
            98, 226, 54, 46, 38, 89, 132, 6, 122, 192, 196, 25, 94, 185, 81, 176, 216, 236, 184,
            224, 222, 126, 225, 205, 75, 81, 57, 156, 168, 112, 1, 109, 221, 94, 59, 78, 130, 195,
            175, 210, 115, 174, 241, 30, 214, 253, 79, 241, 187, 103, 250, 55, 12, 147, 187, 82,
            214, 122, 160, 45, 116, 173, 113, 125, 122, 55, 190, 74, 147, 10, 94, 149, 245, 44,
            165, 3, 191, 73,
        ],
    ];

    let sig: [u8; BLS_SIG_LEN] = [
        128, 121, 139, 21, 70, 47, 71, 10, 140, 249, 105, 241, 123, 149, 1, 141, 216, 30, 74, 215,
        132, 241, 187, 65, 237, 199, 167, 94, 31, 222, 223, 109, 14, 145, 159, 98, 109, 133, 213,
        252, 118, 140, 128, 179, 91, 117, 217, 229, 19, 56, 230, 44, 62, 175, 161, 136, 223, 139,
        169, 161, 204, 104, 192, 74, 124, 45, 91, 136, 11, 191, 53, 202, 210, 135, 41, 160, 199,
        255, 107, 98, 100, 207, 63, 75, 188, 34, 162, 170, 237, 188, 68, 170, 53, 11, 200, 124,
    ];

    // Assert that `sdk::crypto::verify_signature` succeeds for BLS signatures.
    let res = {
        let sig = Signature::new_bls(sig.to_vec());
        let addrs: Vec<Address> = pub_keys
            .iter()
            .map(|pub_key| Address::new_bls(pub_key).unwrap())
            .collect();
        let digests: Vec<&[u8]> = digests.iter().map(|digest| digest.as_slice()).collect();
        sdk::crypto::verify_signature(&sig, &addrs, &digests)
    };
    assert_eq!(res, Ok(true));

    // Assert that bls validation syscall succeeds.
    let res = sdk::crypto::verify_bls_aggregate(&sig, &pub_keys, &digests);
    assert_eq!(res, Ok(true));

    // Both BLS signatures and digests are a G2 point, thus we can use a valid digest's bytes as the
    // G2 bytes for an incorrect signature (and vice versa).
    let invalid_sig = digests[0];
    let invalid_digests = [sig, digests[1], digests[2]];

    // Assert that bls validation syscall fails for an invalid aggregate signature.
    let res = sdk::crypto::verify_bls_aggregate(&invalid_sig, &pub_keys, &digests);
    assert_eq!(res, Ok(false));

    // Assert that bls validation syscall fails for an invalid message digest.
    let res = sdk::crypto::verify_bls_aggregate(&sig, &pub_keys, &invalid_digests);
    assert_eq!(res, Ok(false));

    // Assert that bls validation syscall fails for an invalid public key.
    let invalid_pub_keys = [pub_keys[0], pub_keys[0], pub_keys[2]];
    let res = sdk::crypto::verify_bls_aggregate(&sig, &invalid_pub_keys, &digests);
    assert_eq!(res, Ok(false));
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
    assert_eq!(sdk::network::version(), NetworkVersion::V21);
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

/// Test to make sure we can return into unaligned pointers. Technically, we use repr-packed
/// everywhere so this should always work, but we should test anyways.
fn test_unaligned() {
    unsafe {
        #[link(wasm_import_module = "network")]
        extern "C" {
            #[link_name = "context"]
            fn context_raw(out: *mut NetworkContext) -> u32;
        }

        #[repr(packed, C)]
        struct Unaligned {
            _padding: u8,
            ctx: NetworkContext,
        }
        let mut unaligned: Unaligned = std::mem::zeroed();
        assert_eq!(context_raw(ptr::addr_of_mut!(unaligned.ctx)), 0);
        let out_ptr = ptr::addr_of!(unaligned.ctx);
        let actual: NetworkContext = ptr::read_unaligned(out_ptr);
        let expected = context().unwrap();
        assert_eq!(expected, actual);
    }
}

fn test_upgrade() {
    // test that calling `upgrade_actor` on ourselves results in a SYS_INVALID_RECEIVER error
    // since we don't have a upgrade endpoint
    let code_cid = sdk::actor::get_actor_code_cid(&Address::new_id(10000)).unwrap();
    let res = sdk::actor::upgrade_actor(&code_cid, None).unwrap();

    assert_eq!(
        res.exit_code,
        fvm_shared::error::ExitCode::SYS_INVALID_RECEIVER,
    );
}
