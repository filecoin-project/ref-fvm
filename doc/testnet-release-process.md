# FVM M2 Test Network Release Process

Deploy a test network, you'll need to:

1. Release the FVM.
2. Release the actors (updating the FVM if necessary).
3. Update the FVM in the FFI.
4. Update the FFI & actors in lotus.

## Release the FVM

Cut an _alpha_ release of the FVM (`filecoin-project/ref-fvm`) per the [release process](https://github.com/filecoin-project/ref-fvm/blob/master/CONTRIBUTING.md#releasing).

If the builtin-actors isn't using the latest version of version of `fvm_shared`, `fvm_sdk`, or any of the `ipld/` crates:

1. Clone the `filecoin-project/builtin-actors` repo.
2. Checkout the `next` branch.
3. Update the `fvm*` crates (`cargo upgrade --workspace fvm fvm_sdk ...`).
4. Make a PR, wait for CI, merge, etc.

## Release the Actors

Create a `dev/$date-m2` [pre-release][releases] of the  `next` branch. This will run the [release workflow][release-workflow] and will build a new bundle.

- There's no need to bump the actor crate versions, just cut a release "tag" on github.
- Creating a git tag isn't sufficient, you'll need to create the release on github to actually trigger the build.

## Update the FFI

1. Clone `filecoin-project/filecoin-ffi`, checkout the `feat/m2` branch.
2. Change to the `rust/` directory, and update all FVM crates: `cargo upgrade fvm fvm_shared fvm_ipld_blockstore fvm_ipld_encoding`.
7. Make a PR to `feat/m2`, etc.

## Update Lotus

First, update and rebuild the FFI:

1. Clone `filecoin-project/lotus`, checkout the `experimental/fvm-m2`branch.
2. Change to the `extern/filecoin-ffi` submodule, `git fetch`, then checkout the `experimental/fvm-m2` branch.
3. Run `make FFI_BUILD_FROM_SOURCE=1` inside `extern/filecoin-ffi` to build it. You may need to install some build dependencies, take a look at the FFI README.

Then, update the actors:

1. Change directory to `build/actors` in lotus.
2. Run `./pack.sh v8 dev/$date-m2` (where `dev/$date-m2` is the actor's release tag you just created).

Build and test lotus:

1. Build lotus (run `make` in the repo root).
2. Try to start a [devnet](https://lotus.filecoin.io/lotus/developers/local-network/).

Then finally, make a PR to `experimental/fvm-m2` with your changes.

[release-workflow]: https://github.com/filecoin-project/builtin-actors/actions/workflows/release.yml
[releases]: https://github.com/filecoin-project/builtin-actors/releases
