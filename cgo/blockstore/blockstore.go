package blockstore

import (
	"sync"
	"sync/atomic"
	"unsafe"

	"github.com/filecoin-project/lotus/blockstore"
)

const (
	ErrNoStore = -1 - iota
	ErrNotFound
	ErrIO
)

// Design:
//
// The goal of this library is to expose go-based blockstores over a C-FFI as efficiently as
// possible (without too many hacks). Importantly, this means avoiding locks in the happy path.
//
// To do this, we make liberal use of unsafe pointers and atomic operations such that looking up a
// blockstore requires exactly one atomic read. This means that this API can be _very_ dangerious if
// used incorrectly.
//
// NOTE: This is totally a premature optimization and we may need to ditch all of it. I got a bit
// carried away.

var (
	// "snapshot" of the registered blockstores for atomic access.
	// Well, sort of a snapshot. The backing slice is quite mutable, however:
	// 1. The slice _header_ isn't mutable.
	// 2. Only _free_ slots in the backing slice will be modified.
	atomicRegistry unsafe.Pointer // *[]blockstore.Blockstore

	mu       sync.Mutex
	registry []blockstore.Blockstore // slice of registered blockstores.
	freelist []int                   // a "linked list" of free slots in the registry.
	lastIdx  int                     // the "head" of the freelist.
)

func init() {
	atomic.StorePointer(&atomicRegistry, unsafe.Pointer(new([]blockstore.Blockstore)))
}

// Register a new blockstore and get a handle.
func Register(bs blockstore.Blockstore) int32 {
	mu.Lock()
	defer mu.Unlock()

	idx := lastIdx
	if lastIdx >= len(registry) {
		// We don't need atomics here because we're writing to a "new" section of the registry.
		registry = append(registry, bs)
		freelist = append(freelist, len(freelist))
		lastIdx = len(freelist)
		idx = lastIdx - 1
	} else {
		// We don't need an atomic here because the index is free.
		lastIdx = freelist[idx]
		registry[idx] = bs
	}

	registry := registry // explicitly copy so we get a new slice header.
	atomic.StorePointer(&atomicRegistry, unsafe.Pointer(&registry))
	return int32(idx)
}

// Unregister a blockstore.
//
// WARNING: This method must be called at most _once_ with a handle previously returned by Register.
// Furthermore, it must not be called concurretnly with a Lookup of the same handle.
func Unregister(handle int32) {
	mu.Lock()
	defer mu.Unlock()

	freelist[handle] = lastIdx
	lastIdx = int(handle)
	registry[handle] = nil
}

// Lookup a blockstore by handle.
//
// WARNING: This method must be called witha valid handle to avoid undefined behavior. It must be
// called between Register and Unregister, and must not be called concurrently or after
// Unregistering the blockstore.
func Lookup(handle int32) blockstore.Blockstore {
	registry := *(*[]blockstore.Blockstore)(atomic.LoadPointer(&atomicRegistry))

	if int(handle) >= len(registry) {
		return nil
	}

	return registry[handle]
}
