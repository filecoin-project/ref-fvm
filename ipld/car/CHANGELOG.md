# Changelog

Changes to the FVM's CAR implementation.

## [Unreleased]

## 0.7.0 [2023-09-06)

Replace the internal integer-encoding dependency with unsigned-varint. This won't affect users but cleans up our dependency tree a bit.

## 0.7.0 [2023-06-28)

Breaking Changes:

- Update cid/multihash. This is a breaking change as it affects the API.

## 0.6.0 [2022-10-11]

- Bumps `fvm_ipld_encoding` and switches from `cs_serde_bytes` to `fvm_ipld_encoding::strict_bytes`.

## 0.5.0 [2022-08-03]

This release includes several CAR sanity checks and validations.

- Check for unexpected EOF when reading a CAR file.
- Validate the blocks in the car file by default.
- Allocate at most 1MiB up-front when reading blocks from the CAR.

BREAKING CHANGE: This will now reject CAR files with invalid CIDs by default. Call
`load_car_unchecked` to restore the old behavior.

## 0.4.1

- Update `fvm_ipld_encoding` to 0.2.0.
