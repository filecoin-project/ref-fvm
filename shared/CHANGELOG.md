# Changelog

## 3.0.0-alpha.3 [UNRELEASED]

## 3.0.0-alpha.2 [2022-09-16]

- Add basic f4 address support (without checksums for now).
- Change TokenAmount::from_whole to take any `Into<BigInt>` parameter.
- Add nv17 to the network versions.

The only breaking change is the change to `Address`/`Protocol` (in case anyone is exhaustively matching on them).

## 3.0.0-alpha.1 [2022-08-31]

- Bump base version to v3.
- Add `origin` to `vm::Context`.

## 2.0.0...

See the `release/v2` branch.

- Add recover secp public key syscall.
- Removed `actor::builtin::Type` (moved to the actors themselves).
- Add additional hash functions to the hash syscall.
- Add blake2b512
- Change TokenAmount from a type alias to a struct wrapping BigInt

## 0.8.0 [2022-06-13]

- Add a new proofs version type.

## 0.7.1 [2022-05-26]

Add a shared `MAX_CID_LEN` constant.

## 0.7.0 [2022-05-16]

- Updates the blockstore.
- Removes unnecessary chrono dep.
- Removes the `DomainSeparationTag` type. This is moving into the actors themselves as the FVM
  doesn't care about it.
      - Downstream crates should just replicate this type internally, if necessary.
- Adds a new `crypto::signature::verify` function to allow verifying signatures without creating a
  new `Signature` object. This allows verifying _borrowed_ signatures without allocating.
- Updates for the syscall refactor (see `fvm_sdk` v0.7.0):
    - Adds a `BufferTooSmall` `ErrorNumber`.
    - Marks `ErrorNumber` as non-exhaustive for future extension.
    - Changes the syscall "out" types for the syscall refactor.

## 0.6.1 [2022-04-29]

- Added `testing` feature to have `Default` derive on `Message`. Extended this feature to `Address` and `Payload`.
- Improve `ErrorNumber` documentation.
- Update `fvm_ipld_encoding` for the cbor encoder switch.

## 0.6.0 [2022-04-14]

BREAKING: Switch syscall struct alignment: https://github.com/filecoin-project/fvm-specs/issues/63

Actors built against this new version of fvm_shared will be incompatible with prior FVM versions,
and vice-versa.

- Added `Display` trait to `Type` for error printing.
- Added _cfg = "testing"_ on `Default` trait for `Message` structure.

## 0.5.1  [2022-04-11]

Add the `USR_ASSERTION_FAILED` exit code.

## 0.5.0 [2022-04-11]

- Enforce maximum big-int size to match lotus.
- Make signature properties public.
- Major error type refactor.

The largest change here is a major error type refactor.

1. It's now a u32 with a set of pre-defined values instead of an enum.
2. The error codes have been reworked according to the FVM spec.

Both of these changes were made to better support user-defined actors.
