package extern

import (
	"github.com/filecoin-project/lotus/chain/vm"
	"sync"
	"sync/atomic"
	"unsafe"
)

const (
	ErrNoExtern = -1 - iota
)

var (
	// "snapshot" of the registered runtimes for atomic access.
	// Well, sort of a snapshot. The backing slice is quite mutable, however:
	// 1. The slice _header_ isn't mutable.
	// 2. Only _free_ slots in the backing slice will be modified.
	atomicRegistry unsafe.Pointer // *[]runtime.Runtime

	mu       sync.Mutex
	registry []vm.Extern // slice of registered runtimes.
	freelist []int       // a "linked list" of free slots in the registry.
	lastIdx  int         // the "head" of the freelist.
)

func init() {
	atomic.StorePointer(&atomicRegistry, unsafe.Pointer(new([]vm.Extern)))
}

// Register a new runtime and get a handle.
func Register(bs vm.Extern) int32 {
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

// Unregister a runtime.
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

// Lookup a runtime by handle.
//
// WARNING: This method must be called witha valid handle to avoid undefined behavior. It must be
// called between Register and Unregister, and must not be called concurrently or after
// Unregistering the runtime.
func Lookup(handle int32) vm.Extern {
	registry := *(*[]vm.Extern)(atomic.LoadPointer(&atomicRegistry))

	if int(handle) >= len(registry) {
		return nil
	}

	return registry[handle]
}
