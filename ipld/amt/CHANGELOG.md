# Changelog

## [Unreleased]

## 0.7.4 [2025-04-09]

- Updates multiple dependencies (semver breaking internally but not exported).

## 0.7.3 [2024-11-20]

- Fix a bug where the new `iter()` method would panic or overflow in some cases when iterating past the end of the AMT when the AMT stored high keys.

## 0.7.2 [2024-11-20]

- Un-deprecate `.for_each(...)` and related functions. The `.iter()` method is still preferred but `.for_each(...)` is still useful.

## 0.7.1 [2024-11-08]

Remove unnecessary features from `multihash-codetable`.

## 0.7.0 [2024-10-31]

- Update `cid` to v0.11 and `multihash` to v0.19.
- Update to `fvm_ipld_blockstore` 0.3.0 and `fvm_ipld_encoding` 0.5.0.

You will have to update your multihash and cid crates to be compatible, see the [multihash release notes](https://github.com/multiformats/rust-multihash/blob/master/CHANGELOG.md#-2023-06-06) for details on the breaking changes.

## 0.6.2 [2023-09-28)

Fix a bug in `for_each_ranged` if the start offset exceeds the max possible value in the AMT (due to the AMT's height).

## 0.6.1 [2023-07-06)

Add the ability to efficiently diff two AMTs by calling the `diff` function in the root of the crate (thanks to @hanabi1224).

## 0.6.0 [2023-06-28)

Breaking Changes:

- Update cid/multihash. This is a breaking change as it affects the API.

## 0.5.1

Avoid flushing the AMT if nothing has changed.

## 0.5.0

- Bumps `fvm_ipld_encoding` and switches from `cs_serde_bytes` to `fvm_ipld_encoding::strict_bytes`.
- Remove `ahash` and just use a vec.

## 0.4.2

- Return the correct value from `batch_delete`.

## 0.4.1

- Update `fvm_ipld_encoding` to 0.2.0.
