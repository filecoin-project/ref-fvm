# Changelog

Changes to the reference FVM's HAMT implementation.

## [Unreleased]

## 0.10.4 [2025-04-09]

- Updates multiple dependencies (semver breaking internally but not exported).

## 0.10.3 [2024-12-04]

- Add a `.clear()` method for resetting the HAMT to empty.

## 0.10.2 [2024-11-20]

Empty-release intended to un-deprecate `.for_each` but it was never actually deprecated in this crate.

## 0.10.1 [2024-11-08]

Remove unnecessary features from `multihash-codetable`.

## 0.10.0 [2024-10-31]

- Update `cid` to v0.11 and `multihash` to v0.19.
- Update to `fvm_ipld_blockstore` 0.3.0 and `fvm_ipld_encoding` 0.5.0.
- Switch from [libipld](https://github.com/ipld/libipld) to [rust-ipld-core](https://github.com/ipld/rust-ipld-core/).

You will have to update your multihash and cid crates to be compatible, see the [multihash release notes](https://github.com/multiformats/rust-multihash/blob/master/CHANGELOG.md#-2023-06-06) for details on the breaking changes.

## 0.9.0 (2023-10-25)

Breaking Changes:

- Remove the `ignore-dead-links` feature.
- Add a new `StartKeyNotFound` variant to the HAMT error type.

Features:

- Implement external iteration via `iter()` and `iter_from(start_key)`.

Fixes:

- Extra-paranoid validation when reading HAMT nodes.

## 0.8.0 [2023-08-18)

Breaking Changes:

- Deprecate default bitwidths in the HAMT
  - Users must now always specify the bitwidth
- Add support for the Hamt version 0 datastructure, for historical purposes.

## 0.7.0 [2023-06-28)

Breaking Changes:

- Update cid/multihash. This is a breaking change as it affects the API.
- Add `min_data_depth` option to reserve the top levels of the HAMT for links, free of key-value pairs.

## 0.6.1 [2022-11-14]

- FIX: HashBits::next when bit_width does not divide 256 and the full hash is consumed

## 0.6.0

- Bumps `fvm_ipld_encoding` and switches from `cs_serde_bytes` to `fvm_ipld_encoding::strict_bytes`.

## 0.5.1

- Update `fvm_ipld_encoding` to 0.2.0.

## 0.5.0

- BREAKING: update fvm_shared to 0.5.1 for error refactor.
- BREAKING: rename `Hamt::consume` to `Hamt::into_store`.
