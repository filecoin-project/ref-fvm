# Changelog

## [Unreleased]

- Added _cfg = "testing"_ on `testing` module.
- Added a `testing` module to access `assert_*` macros to be able to do assertions in actors code.

## 0.6.0 [2022-04-14]

BREAKING: Upgrades to fvm_shared 0.6.0, and the new syscall struct alignment.
https://github.com/filecoin-project/fvm-specs/issues/63

## 0.5.0 [2022-04-11]

Upgrades the SDK to fvm_shared 0.5.0. This release includes a significant breaking change to exit codes.