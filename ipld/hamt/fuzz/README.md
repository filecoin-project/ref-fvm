# Fuzz testing

## Prerequisites

Install the [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) extension.

## Running the tests

```bash
cargo fuzz run <test-name>
```

For example:

```bash
cargo fuzz run extensions
cargo fuzz run extensions fuzz/artifacts/extensions/crash-4b0ad5d0cfb7ac10ab91cd48d9d532c7abd89cc2
```

## Add a new test

Ideally each test should have their own corpus, because the CI uses ClusterFuzz to evolve the set.
The corpora are at https://github.com/filecoin-project/ref-fvm-fuzz-corpora

To initiate a new test, start by copying an existing test set, e.g. `hamt_simple` as `hamt_extensions`, then create a symlink under `corpus` so cargo finds them:

```bash
cd fuzz/corpus
ln -s ../../../../testing/fuzz-corpora/corpus/hamt_extensions extensions
```

This assumes that the above repo is checked out and `ref-fvm/testing/fuzz-corpora` has a symlink set at it.
