# Changelog

## 2.10.0 (2024-11-21)

- Update `cid` to v0.11 and `multihash` to v0.19.
- Update to `fvm_ipld_blockstore` 0.3.0 and `fvm_ipld_encoding` 0.5.0.

You will have to update your multihash and cid crates to be compatible, see the [multihash release notes](https://github.com/multiformats/rust-multihash/blob/master/CHANGELOG.md#-2023-06-06) for details on the breaking changes.

## 2.9.1 (2024-10-21)

- Update to wasmtime 25.

## 2.9.0 (2024-09-12)

- Update misc dependencies.

## 2.7.0 (2024-06-12)

- Update `filecoin-proofs-api` to v18 

## 2.6.0 (2023-09-06)

- BREAKING: Upgrade the proofs API to v16.
- BREAKING (linking): upgrade blstrs to v0.7 and
- BREAKING: update the minimum rust version to 1.70.0

## 2.5.0 (2023-06-28)

Breaking Changes:

- Update all IPLD crates, including cid & multihash.
- Update wasmtime to v10.

## 2.4.0 [2023-05-03]

- Update proofs to v14.
- Misc clippy fixes.

## 2.3.0 [2023-03-09]

Update proofs. Unfortunately, this is a breaking change in a minor release, but we can't cut a v3...

## 2.0.0 [2022-10-12]

No change.

## 2.0.0-alpha.4 [2022-09-12]

- Add nv17 to network version list (non-breaking).
- Allow constructing a token amount from whole "bigints".

## 2.0.0-alpha.3 [2022-08-31]

- Add even more math operations to TokenAmount.
- Add a Sum method to TokenAmount.

## 2.0.0-alpha.2 [2022-08-29]

- Add additional math operations to TokenAmount.

## 2.0.0-alpha.1 [2022-08-29]

- Add recover secp public key syscall.
- Removed `actor::builtin::Type` (moved to the actors themselves).
- Add additional hash functions to the hash syscall.
- Add blake2b512
- Change TokenAmount from a type alias to a struct wrapping BigInt

(lots of breaking changes)

## 0.9.0

- Update proofs.

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
