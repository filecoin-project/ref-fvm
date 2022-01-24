module github.com/filecoin-project/ref-fvm/examples/blockstore/owner

go 1.16

require (
	github.com/filecoin-project/fvm/cgo v0.0.0-20211123180800-6a886fff748d
	github.com/filecoin-project/lotus v1.13.3-0.20220125024031-8ca2b9ef02ce
	github.com/ipfs/go-block-format v0.0.3
	github.com/ipfs/go-cid v0.1.0
	github.com/multiformats/go-multihash v0.1.0
)

replace github.com/filecoin-project/fvm/cgo => ../../../cgo
