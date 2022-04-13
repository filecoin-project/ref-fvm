# Changelog

## [Unreleased]

## 0.5.1  [2022-04-11]

Add the `USR_ASSERTION_FAILED` exit code.

## 0.5.0 [2022-04-11]

- Enforce maximum big-int size to match lotus.
- Make signature properties public.
- Major error type refactor.

The largest change here is a major error type refactor.

1. It's now a u32 with a set of pre-defined values instead of an enum.
2. The error codes have been reworked according to the FVM spec.

Both of these changes were made to better support user-defined actors.
