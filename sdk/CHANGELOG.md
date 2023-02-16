# Changelog

## [Unreleased]

## 3.0.0-alpha.24 [2023-02-06]

- Update fvm shared for event changes.

## 3.0.0-alpha.23 [2023-02-01]

- feat: use a struct for network versions (#1496)

## 3.0.0-alpha.22 [2023-01-12]

- Refactor: Move Response from SDK to shared

## 3.0.0-alpha.21 [2023-01-11]

- Refactor: exit takes Option<IpldBlock>

## 3.0.0-alpha.20 [2023-01-09]

- Remove the Cbor trait and its uses
- Refactor: `send` uses `Option<IpldBlock>` for return value
  - `Send` no longer returns `Receipts`
  - Instead, a new `Response` type captures the exit code and optional return data

## 3.0.0-alpha.19 [2022-12-17]

- feat: only store delegated addresses in the state-tree
  - Renames `lookup_address` to `lookup_delegated_address`, and only returns f4 addresses

## 3.0.0-alpha.18 [2022-12-14]

- Refactor: ChainID was moved from FVM to shared

## 3.0.0-alpha.17 [2022-12-08]

- In send, change 0 gas to mean 0 gas (not unlimited).

## 3.0.0-alpha.16 [2022-12-07]

- Remove GasLimit from the message context.
- Add the message nonce to the message context
- Add the chain ID to the network context.
- Unify the send functions into a single function.

## 3.0.0-alpha.15 [2022-11-29]

- Send: handle non-zero exit return values 
  - Returned values are read even if the Send had a non-zero exit code
- Send syscall: add an optional gas limit
- Add a read-only mode to Sends
  - Adds "flags" to the Send syscall, more such flags can be added later as needed.

## 3.0.0-alpha.14 [2022-11-18]

- Replace `new_actor_address` with `next_actor_address`. `next_actor_address` has no side effects (until the actor is actually created).
- Replace `abort` with a generalized `exit` syscall. This allows actors to return values on abort.

## 3.0.0-alpha.13 [2022-11-17]

- Re-export a tipset_timestamp function.
- Remove the imports for removed syscalls.

## 3.0.0-alpha.12 [2022-11-17]

- Refactor network/message contexts to reduce the number of syscalls.

## 3.0.0-alpha.11 [2022-11-15]

- Add support for actor events (FIP-0049).

## 3.0.0-alpha.10 [2022-11-14]

- Split `InvokeContext` into two (#1070)

## 3.0.0-alpha.9 [2022-10-21]

- When debugging is enabled, set the default actor log level to trace. This won't affect actors unless debugging is enabled.

## 3.0.0-alpha.8 [2022-10-21]

- Fix address buffer length in new_actor_address and lookup_address.

## 3.0.0-alpha.7 [2022-10-21]

- Dependency upgrades.

## 3.0.0-alpha.6 [2022-10-20]

- Dependency upgrades.

## 3.0.0-alpha.5 [2022-10-10]

- Bumps `fvm_ipld_encoding` and switches from `cs_serde_bytes` to `fvm_ipld_encoding::strict_bytes`.

## 3.0.0-alpha.4 [2022-10-10]

- Add support for recording & looking up f4 addresses.

## 3.0.0-alpha.3 [2022-10-10]

- Rust 2021 edition.
- Add a new `balance_of` syscall.
- Add a new tipset_cid syscall.
- Add a new timestamp syscall.
- Add an sdk "initialization" helper (for initializing logging, panic handlers, etc.).
- Removes custom assert macros (initialize error handling instead).
- Add syscalls to get the gas limit and premium.
- Add a syscall to get the available gas.

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
