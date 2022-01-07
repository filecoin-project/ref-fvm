use crate::sys;
use crate::SyscallResult;
use cid::Cid;
use fvm_shared::address::Address;

/// Signals that this actor accepts calls from any other actor.
pub fn validate_immediate_caller_accept_any() -> SyscallResult<()> {
    unsafe { sys::validation::validate_immediate_caller_accept_any() }
}

/// Validates that the call being processed originated at one
/// of the listed addresses.
///
/// The list of addreses is provided as a CBOR encoded list.
pub fn validate_immediate_caller_addr_one_of(addrs: &[Address]) -> SyscallResult<()> {
    // TODO error handling during decoding, although this is likely a fatal error.
    unsafe {
        let v = addrs.to_vec();
        let encoded: Vec<u8> = fvm_shared::encoding::to_vec(&v).unwrap();
        sys::validation::validate_immediate_caller_addr_one_of(
            encoded.as_ptr(),
            encoded.len() as u32,
        )
    }
}

/// Validates that the call being processed originated at an
/// actor of one of the specified types.
///
/// The list of CIDs is provided as a CBOR encoded list.
pub fn validate_immediate_caller_type_one_of(cids: &[Cid]) -> SyscallResult<()> {
    // TODO error handling during decoding, although this is likely a fatal error.
    unsafe {
        let v = cids.to_vec();
        let encoded: Vec<u8> = fvm_shared::encoding::to_vec(&v).unwrap();
        sys::validation::validate_immediate_caller_type_one_of(
            encoded.as_ptr(),
            encoded.len() as u32,
        )
    }
}
