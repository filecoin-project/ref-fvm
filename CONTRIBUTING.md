# Contributing

This document describes the process of contributing to the reference FVM implementation (this project).

## Issues

If you have a general FVM related question or idea, please either as on the [Filecoin Slack][chat], or open
a new discussion in [fvm-specs][discuss].

If you'd like to report a _bug_ or suggest an enhancement in the reference FVM implementation, please file [an issue][issue].

## Pull Requests

To make a change to the FVM.

1. When in doubt, open an [issue][] first to discuss the change.
2. Make your change.
3. Write a test for your change.
4. Update the crate's `CHANGELOG.md`. If you're making any breaking changes, prefix change with
   "BREAKING:".
5. Finally, open a PR.

## Releasing

To prepare a release.

1. Pick a next version (for each crate you're releasing). Look through the changelogs and carefully
   decide if it's a breaking release or not. Then read [rust's semver
   documentation](https://doc.rust-lang.org/cargo/reference/semver.html).
2. Install `cargo-edit` (`cargo install cargo-edit`).
3. Create a new branch.
4. Use `cargo set-version` to set the version for each crate you're releasing. This will both update
   the crate version, and make all other crates in the workspace depend on the latest version.
5. Make sure the changelogs are all up-to-date, set the release date & version, and add a new
   "Unreleased" section.
6. Commit your changes, and make a PR.

Once the release is prepared, it'll go through a review:

1. Make sure that we're _ready_ to release. E.g., make sure downstream can consume the release.
2. Make sure that we're correctly following semver.
3. Make sure that we're not missing anything in the changelogs.

Finally, an FVM "owner" will:

1. Merge the release PR to master.
2. For each released crate, create a git: `crate_name@crate_version`.
3. Run `cargo publish` for each released crate (in dependency order).

[chat]: https://docs.filecoin.io/about-filecoin/chat-and-discussion-forums/#chat
[discuss]: https://github.com/filecoin-project/fvm-specs/discussions
[issue]: https://github.com/filecoin-project/ref-fvm/issues
