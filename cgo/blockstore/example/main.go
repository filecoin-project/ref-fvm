package main

// unresolved-symbols: https://github.com/golang/go/issues/14985
// Otherwise, rust can't find our exported functions.

/*
#include <stdint.h>
#include "./libcgobs_example.h"
#cgo LDFLAGS: -L. -lcgobs_example -lm -ldl -Wl,-unresolved-symbols=ignore-all
*/
import "C"
import (
	cgobs "github.com/filecoin-project/fvm/cgo/blockstore"
	"github.com/filecoin-project/lotus/blockstore"
)

func write_blocks(bs blockstore.Blockstore, count int) {
	store := cgobs.Register(bs)
	_ = C.write_blocks(C.int32_t(store), C.int32_t(count))
}

func read_blocks(bs blockstore.Blockstore, count int) {
	store := cgobs.Register(bs)
	_ = C.read_blocks(C.int32_t(store), C.int32_t(count))
}
