package main

import (
	"testing"

	"github.com/filecoin-project/lotus/blockstore"
	blocks "github.com/ipfs/go-block-format"
	"github.com/ipfs/go-cid"
	"github.com/multiformats/go-multihash"
)

var builder = cid.V1Builder{Codec: 0x55, MhType: multihash.SHA2_256, MhLength: -1}

func BenchmarkWriteCgo(b *testing.B) {
	bs := blockstore.NewMemory()
	b.ResetTimer()
	write_blocks(bs, b.N)
	if len(bs) != 1 {
		b.Fatal("expected one element")
	}
}

func BenchmarkWriteDirect(b *testing.B) {
	bs := blockstore.NewMemory()
	data := []byte("thing")
	k, _ := builder.Sum(data)
	block, _ := blocks.NewBlockWithCid(data, k)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = bs.Put(block)
	}
	if len(bs) != 1 {
		b.Fatal("expected one element")
	}
}

func BenchmarkReadCgo(b *testing.B) {
	bs := blockstore.NewMemory()
	data := []byte("thing")
	k, _ := builder.Sum(data)
	block, _ := blocks.NewBlockWithCid(data, k)
	bs.Put(block)
	b.ResetTimer()
	read_blocks(bs, b.N)
}

func BenchmarkReadDirect(b *testing.B) {
	bs := blockstore.NewMemory()
	data := []byte("thing")
	k, _ := builder.Sum(data)
	block, _ := blocks.NewBlockWithCid(data, k)
	bs.Put(block)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		blk, _ := bs.Get(block.Cid())
		if len(blk.RawData()) != len(data) {
			b.Fatal("wrong size")
		}
	}
	if len(bs) != 1 {
		b.Fatal("expected one element")
	}
}
