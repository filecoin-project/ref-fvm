package blockstore

import (
	"context"
	"unsafe"

	"github.com/filecoin-project/lotus/blockstore"
	blocks "github.com/ipfs/go-block-format"
	"github.com/ipfs/go-cid"
)

/*
#include <stdint.h>
typedef const uint8_t* buf_t;
*/
import "C"

func toCid(k C.buf_t, k_len C.int32_t) cid.Cid {
	return *(*cid.Cid)(unsafe.Pointer(&struct{ str string }{str: C.GoStringN((*C.char)(unsafe.Pointer(k)), C.int(k_len))}))
}

//export cgobs_get
func cgobs_get(store C.int32_t, k C.buf_t, k_len C.int32_t, block **C.uint8_t, size *C.int32_t) C.int32_t {
	c := toCid(k, k_len)
	bs := Lookup(int32(store))
	if bs == nil {
		return ErrNoStore
	}
	err := bs.View(context.Background(), c, func(data []byte) error {
		*block = (C.buf_t)(C.CBytes(data))
		*size = C.int32_t(len(data))
		return nil
	})

	switch err {
	case nil:
		return 0
	case blockstore.ErrNotFound:
		return ErrNotFound
	default:
		return ErrIO
	}
}

//export cgobs_put
func cgobs_put(store C.int32_t, k C.buf_t, k_len C.int32_t, block C.buf_t, block_len C.int32_t) C.int32_t {
	c := toCid(k, k_len)
	bs := Lookup(int32(store))
	if bs == nil {
		return ErrNoStore
	}
	b, _ := blocks.NewBlockWithCid(C.GoBytes(unsafe.Pointer(block), C.int(block_len)), c)
	if bs.Put(context.Background(), b) != nil {
		return ErrIO
	}
	return 0
}

// TODO: Implement a "put many". We should just pass a single massive buffer, or an array of
// buffers?

//export cgobs_delete
func cgobs_delete(store C.int32_t, k C.buf_t, k_len C.int32_t) C.int32_t {
	c := toCid(k, k_len)
	bs := Lookup(int32(store))
	if bs == nil {
		return ErrNoStore
	}
	if bs.DeleteBlock(context.Background(), c) != nil {
		return ErrIO
	}
	return 0
}

//export cgobs_has
func cgobs_has(store C.int32_t, k C.buf_t, k_len C.int32_t) C.int32_t {
	c := toCid(k, k_len)
	bs := Lookup(int32(store))
	if bs == nil {
		return ErrNoStore
	}
	has, err := bs.Has(context.Background(), c)
	switch err {
	case nil:
	case blockstore.ErrNotFound:
		// Some old blockstores still return this.
		return 0
	default:
		return ErrIO
	}
	if has {
		return 1
	}
	return 0
}
