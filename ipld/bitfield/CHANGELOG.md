# Changelog

Changes to Filecoin's Bitfield library.

## [Unreleased]

## 0.3.1 [2024-11-08]

Remove unnecessary features from `multihash-codetable`.

## 0.3.0 [2024-10-31]

- Update `cid` to v0.11 and `multihash` to v0.19.
- Update to `fvm_ipld_blockstore` 0.3.0 and `fvm_ipld_encoding` 0.5.0.

You will have to update your multihash and cid crates to be compatible, see the [multihash release notes](https://github.com/multiformats/rust-multihash/blob/master/CHANGELOG.md#-2023-06-06) for details on the breaking changes.

## 0.6.0 [2023-08-31]

- Bumps `fvm_ipld_encoding` to 0.4.0, and `fvm_ipld_blockstore` to 0.2.0.

## 0.5.4 [2022-10-11]

- Bumps `fvm_ipld_encoding` and switches from `cs_serde_bytes` to `fvm_ipld_encoding::strict_bytes`.

## 0.5.3 [2022-09-12]

- Optimize no-op operations.

## 0.5.2 [2022-05-16]

- Check size of `UnvalidatedBitfield`.

## 0.5.1 [2022-04-29]

- Update `fvm_ipld_encoding`.
