use fvm_sdk as sdk;
use fvm_shared::crypto::hash::SupportedHashes;
use fvm_shared::error::ExitCode;

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

    #[cfg(coverage)]
    sdk::debug::store_artifact("syscall_actor.profraw", minicov::capture_coverage());
    0
}

// use SDK methods to hash and compares against locally (inside the actor) hashed digest
fn test_expected_hash() {
    let test_bytes = b"foo bar baz boxy";
    

    let blake_local = SupportedHashes::Blake2b256.digest(test_bytes).digest();
    let blake_arr = sdk::crypto::hash_blake2b(test_bytes); // test against old SDK method since it does less unsafe things
    let blake_vec = sdk::crypto::hash(SupportedHashes::Blake2b256, test_bytes);

    assert_eq!(blake_arr.as_slice(), blake_vec.as_slice());
    assert_eq!(blake_local, blake_vec.as_slice());

    expected_digest!(SupportedHashes::Sha2_256, test_bytes);
    expected_digest!(SupportedHashes::Keccak256, test_bytes);
    expected_digest!(SupportedHashes::Ripemd160, test_bytes);
}

macro_rules! expected_digest {
    ($hasher:expr, $data:expr) => {{
        // scoped so we can re-use variable names
        let local_digest = $hasher.digest($data).digest();
        let digest = sdk::crypto::hash($hasher, $data);

        assert_eq!(local_digest, digest.as_slice());
    }};
}

// do funky things with hash syscall directly
fn test_hash_syscall() {
    // TODO
}
