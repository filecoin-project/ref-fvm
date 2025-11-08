// Placeholder for VM-level EXTCODE* projection tests.
// These require exercising the EVM actor inside a full DefaultCallManager stack,
// which is non-trivial in this test harness. Kept as an ignored test stub to lock
// down intent; coverage exists in builtin-actors EVM tests.

#[test]
#[ignore]
fn evm_extcode_projection_size_hash_copy() {
    // Intended assertions:
    // - EXTCODESIZE(A) == 23 when EthAccount(A).delegate_to is set
    // - EXTCODECOPY(A,0,0,23) returns 0xEF 0x01 0x00 || delegate(20)
    // - EXTCODEHASH(A) equals keccak(pointer_code)
    // Implementation to follow with an integration harness using DefaultCallManager.
}

