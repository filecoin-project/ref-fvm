# Changelog

## [Unreleased]

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
