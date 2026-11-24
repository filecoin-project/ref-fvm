# IPLD crates (ipld/)

These crates hold actor state for every Filecoin node, and builtin-actors, filecoin-ffi and Forest depend on them directly. They follow ordinary semver and are released independently of the primary crates, from master only.

- `hamt`, `amt` and `kamt` must produce blocks bit for bit identical to [go-hamt-ipld](https://github.com/filecoin-project/go-hamt-ipld), [go-amt-ipld](https://github.com/filecoin-project/go-amt-ipld) and [go-kamt-ipld](https://github.com/filecoin-project/go-kamt-ipld). Lotus reads and migrates state with the Go implementations while actors write it with these, so a divergence is a consensus failure. A change to encoding, node layout, bucket ordering or canonical form needs the matching Go change and differential fixtures.
- Configuration (bit width, bucket size) lives outside the blocks, except the AMT bit width. A reader must know the shape it is reading; don't change a default that existing state relies on.
- Actors pay gas for every block they load and write through these crates. Changing how many blocks an operation touches, or in what order, changes gas even when the result is identical. Add new traversal behaviour as an opt-in method beside the existing one.
- `encoding` and `blockstore` traits are shared across each host's Cargo graph, including the release branches, which take these crates from crates.io; see Backports and crate coexistence in the root `AGENTS.md` before a breaking release.

Release steps: `RELEASE.md` (Preparing Other/Non-Primary Crates).
