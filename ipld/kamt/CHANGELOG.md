# Changelog

## 0.4.0 [2024-10-31]

- Update `cid` to v0.11 and `multihash` to v0.19.
- Update to `fvm_ipld_blockstore` 0.3.0 and `fvm_ipld_encoding` 0.5.0.

You will have to update your multihash and cid crates to be compatible, see the [multihash release notes](https://github.com/multiformats/rust-multihash/blob/master/CHANGELOG.md#-2023-06-06) for details on the breaking changes.

## 0.3.0 [2023-06-28)

Breaking Changes:

- Update cid/multihash. This is a breaking change as it affects the API.

## 0.2.0 [2023-01-13]

- Improve serialization format by avoiding maps.
- Various performance improvements.

## [Unreleased]

- ...
