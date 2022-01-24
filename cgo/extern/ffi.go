package extern

import "C"
import "unsafe"

//export cgo_extern_get_chain_randomness
func cgo_extern_get_chain_randomness(handle C.int32_t, pers C.int64_t, round C.int64_t, entropy C.buf_t, entropy_len C.int32_t, randomness **C.uint8_t) C.int32_t {

	ext := Lookup(int32(handle))
	if ext == nil {
		return ErrNoExtern
	}

	r := ext.GetRandomnessFromTickets(C.int32_t(len(pers)), C.int32_t(len(round)), C.GoBytes(unsafe.Pointer(entropy), C.int(entropy_len)))

	*randomness = (C.buf_t)(C.CBytes(r))

	return 0
}

//export cgo_extern_get_beacon_randomness
func cgo_extern_get_beacon_randomness(handle C.int32_t, pers C.int64_t, round C.int64_t, entropy C.buf_t, entropy_len C.int32_t, randomness **C.uint8_t) C.int32_t {

	ext := Lookup(int32(handle))
	if ext == nil {
		return ErrNoExtern
	}

	r := ext.GetRandomnessFromBeacon(C.int32_t(len(pers)), C.int32_t(len(round)), C.GoBytes(unsafe.Pointer(entropy), C.int(entropy_len)))

	*randomness = (C.buf_t)(C.CBytes(r))

	return 0
}

//export cgo_extern_verify_consensus_fault
func cgo_extern_verify_consensus_fault(handle C.int32_t, h1 C.buf_t, h1_len C.int32_t, h2 C.buf_t, h2_len C.int32_t, extra C.buf_t, extra_len C.int32_t, target **C.uint8_t, target_size *C.int32_t, epoch *C.int64_t, fault_type *C.uint8_t) C.int32_t {

	ext := Lookup(int32(handle))
	if ext == nil {
		return ErrNoExtern
	}

	targetAddress, faultEpoch, faultType := ext.VerifyConsensusFault(C.GoBytes(unsafe.Pointer(h1), C.int(h1_len)), C.GoBytes(unsafe.Pointer(h2), C.int(h2_len)), C.GoBytes(unsafe.Pointer(extra), C.int(extra_len)))
	*target = (C.buf_t)(C.CBytes(targetAddress))
	*target_size = C.int32_t(len(targetAddress.Bytes()))
	*epoch = C.int32_t(faultEpoch)
	*fault_type = C.uint8_t(faultType)

	return 0
}
