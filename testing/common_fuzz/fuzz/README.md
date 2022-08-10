There are currently 3 fuzzing suites within ref-fvm: 
- `common` at `/testing/common_fuzz/fuzz` 
- `amt` at `/ipld/amt/fuzz/`
- `hamt` at `/ipld/hamt/fuzz/`.

Within the CI, the name of the target is `${SUITE}_${FUZZ_TARGET}`.

You have to have [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) installed. To test a crasher run: `cargo fuzz run $FUZZ_TARGET $CRASHER_FILE`.
