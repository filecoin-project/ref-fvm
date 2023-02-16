# Changelog

## [Unreleased]

## 0.5.1

Avoid flushing the AMT if nothing has changed.

## 0.5.0

- Bumps `fvm_ipld_encoding` and switches from `cs_serde_bytes` to `fvm_ipld_encoding::strict_bytes`.
- Remove `ahash` and just use a vec.

## 0.4.2

- Return the correct value from `batch_delete`.

## 0.4.1

- Update `fvm_ipld_encoding` to 0.2.0.
