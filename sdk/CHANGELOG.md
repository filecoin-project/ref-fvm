# Changelog

## 2.0.0-alpha.5 [2022-09-12]

Update fvm_shared.

## 2.0.0-alpha.4 [2022-08-31]

Update fvm_shared.

## 2.0.0-alpha.3 [2022-08-29]

Update fvm_shared.

## 2.0.0-alpha.2 [2022-08-29]

- Change randomness return value to a fixed-sized byte array.
- Remove builtin blake2b hashing.
    - Removes `Message::to_signing_bytes`.
    - Removes `Cbor::cid`.
- Remove actor `Type` enum. Instead, use u32 to identify actor types.
- Add a `recover_secp_public_key` syscall.

## 3.0.0-alpha.2 [2022-09-02]

- New `hash_into` sdk for hashing into a mut buffer.
- Rename `hash` to `hash_owned`.

## 3.0.0-alpha.1 [2022-08-31]

- Bump base version to v3.
- New `install_actor` syscall.

## 2.0.0...

- Change randomness return value to a fixed-sized byte array.
- Remove builtin blake2b hashing.
    - Removes `Message::to_signing_bytes`.
    - Removes `Cbor::cid`.
- Remove actor `Type` enum. Instead, use u32 to identify actor types.
- Add a `recover_secp_public_key` syscall.

## 1.0.0 [2022-06-23]

- Update to the latest `fvm_shared`, which supports the new proofs types.
- Fix the ipld get/read method to correctly handle blocks with approximate sizes.
- Use the new shared `MAX_CID_LEN` constant.
- Cleanup some TODOs
- Revert accidental bump of `MAX_CID_LEN` to 256 (back to 100).

## 0.7.0 [2022-05-16]

This release contains a large syscall refactor.

High-level SDK changes:

- Changed randomness methods to take a raw i64 as domain separation tags, instead of the
  `DomainSeparationTag` type.
- Renamed `actor::resolve_builtin_actor_type` to `actor::get_builtin_actor_type`.

Low-level "sys" changes:

- Fetches static information through a single `vm::context` syscall. The high-level calls haven't
  changed and simply call this method on-demand, caching the result. This replaces:
  - `network`: `version`, `curr_epoch`.
  - `message`: `caller`, `receiver`, `method_number`, `value_received`.
- Methods that "return" CIDs now do so consistently:
 - They always return the CID's length.
 - If they can't fit the entire CID into the output buffer, they fail with `ErrorNumber::BufferTooSmall`.
 - Previously:
   - `ipld::root` and `ipld::cid` would return the CID's length even if the output buffer wasn't large enough.
- Renamed IPLD methods:
  - Prefixed with `block_` to make it clear that they operate on IPLD blocks.
  - Renamed `ipld::cid` to `ipld::block_link`.
- Changed the behavior of `ipld::block_read`.
  - Previously, it would return the number of bytes read.
  - Now it returns the difference between the passed `offset + max_len` (end of the "buffer") and
    the end of the block.
- Replaced `crypto::hash_blake2b` with a generic `crypto::hash` syscall that takes a multicodec.
  This syscall will:
    - Hash the input data with the specified hash function (if supported).
    - Write the hash digest into the provided output buffer, truncating if the output buffer is too short.
    - Return the number of bytes written.
- Refactored `crypto::verify_signature` to take a signature type and a raw signature, instead of a
  cbor-encoded signature.
- Changed `actor::get_code_cid_for_type` to return an _error_ if the code CID lookup fails, instead of `-1`.
- Renamed `actor::resolve_builtin_actor_type` to `get_builtin_actor_type`.
- Changed `actor::get_actor_code_cid` to take a pre-resolved `actor_id`. The higher-level helper
  function hasn't changed, and will resolve the actor's address internally, if necessary.
- Changed `actor::resolve_address` to:
  - Fail with a `NotFound` error if the target actor isn't found.
  - Return the target actor's ID instead of a struct containing the ID and a status.
- Changed `send::send`'s return type to include the returned block's size and codec. This let's us
  avoid a call to `ipld::block_stat`.`

## 0.6.1 [2022-04-29]

- Added _cfg = "testing"_ on `testing` module.
- Added a `testing` module to access `assert_*` macros to be able to do assertions in actors code.
- Update `fvm_ipld_encoding` to 0.2.0 for CBOR library switch.

## 0.6.0 [2022-04-14]

BREAKING: Upgrades to fvm_shared 0.6.0, and the new syscall struct alignment.
https://github.com/filecoin-project/fvm-specs/issues/63

## 0.5.0 [2022-04-11]

Upgrades the SDK to fvm_shared 0.5.0. This release includes a significant breaking change to exit codes.
