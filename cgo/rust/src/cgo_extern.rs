use std::{ptr, slice};

// TODO: remove this when we implement these
use anyhow::anyhow;
use fvm_shared::address::Address;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::consensus::ConsensusFault;
use fvm_shared::crypto::randomness::DomainSeparationTag;
use fvm_shared::externs::{Consensus, Externs, Rand};

const ERR_NO_EXTERN: i32 = -1;

extern "C" {
    pub fn cgo_extern_get_chain_randomness(
        handle: i32,
        pers: i64,
        round: i64,
        entropy: *const u8,
        entropy_len: i32,
        randomness: *mut *mut u8,
    ) -> i32;

    pub fn cgo_extern_get_beacon_randomness(
        handle: i32,
        pers: i64,
        round: i64,
        entropy: *const u8,
        entropy_len: i32,
        randomness: *mut *mut u8,
    ) -> i32;

    fn cgo_extern_verify_consensus_fault(
        handle: i32,
        h1: *const u8,
        h1_len: i32,
        h2: *const u8,
        h2_len: i32,
        extra: *const u8,
        extra_len: i32,
        addr_buf: *mut *mut u8,
        addr_size: *mut i32,
        epoch: *mut i64,
        fault: *mut u8,
    ) -> i32;
}

pub struct CgoExterns {
    handle: i32,
}

impl CgoExterns {
    /// Construct a new externs from a handle.
    pub fn new(handle: i32) -> CgoExterns {
        CgoExterns { handle }
    }
}

impl Rand for CgoExterns {
    fn get_chain_randomness(
        &self,
        pers: DomainSeparationTag,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        unsafe {
            let mut buf: *mut u8 = ptr::null_mut();
            match cgo_extern_get_chain_randomness(
                self.handle,
                pers as i64,
                round,
                entropy.as_ptr(),
                entropy.len() as i32,
                &mut buf,
            ) {
                0 => Ok(<[u8; 32]>::try_from(slice::from_raw_parts(buf, 32))?),
                r @ 1.. => panic!("invalid return value from has: {}", r),
                ERR_NO_EXTERN => panic!("extern {} not registered", self.handle),
                e => Err(anyhow!(
                    "cgo extern 'get_chain_randomness' failed with error code {}",
                    e
                )),
            }
        }
    }

    fn get_beacon_randomness(
        &self,
        pers: DomainSeparationTag,
        round: ChainEpoch,
        entropy: &[u8],
    ) -> anyhow::Result<[u8; 32]> {
        unsafe {
            let mut buf: *mut u8 = ptr::null_mut();
            match cgo_extern_get_beacon_randomness(
                self.handle,
                pers as i64,
                round,
                entropy.as_ptr(),
                entropy.len() as i32,
                &mut buf,
            ) {
                0 => Ok(<[u8; 32]>::try_from(slice::from_raw_parts(buf, 32))?),
                r @ 1.. => panic!("invalid return value from has: {}", r),
                ERR_NO_EXTERN => panic!("extern {} not registered", self.handle),
                e => Err(anyhow!(
                    "cgo extern 'get_beacon_randomness' failed with error code {}",
                    e
                )),
            }
        }
    }
}

impl Consensus for CgoExterns {
    fn verify_consensus_fault(
        &self,
        h1: &[u8],
        h2: &[u8],
        extra: &[u8],
    ) -> anyhow::Result<Option<ConsensusFault>> {
        unsafe {
            let mut addr_buf: *mut u8 = ptr::null_mut();
            let mut addr_size: i32 = 0;
            let mut epoch: i64 = 0;
            let mut fault_type: u8 = 0;
            match cgo_extern_verify_consensus_fault(
                self.handle,
                h1.as_ptr(),
                h1.len() as i32,
                h2.as_ptr(),
                h2.len() as i32,
                extra.as_ptr(),
                extra.len() as i32,
                &mut addr_buf,
                &mut addr_size,
                &mut epoch,
                &mut fault_type,
            ) {
                0 => Ok(Some(ConsensusFault {
                    target: Address::from_bytes(slice::from_raw_parts(
                        addr_buf,
                        addr_size as usize,
                    ))?,
                    epoch,
                    fault_type: fault_type
                        .try_into()
                        .map_err(|_| anyhow!("invalid fault type"))?,
                })),
                r @ 1.. => panic!("invalid return value from has: {}", r),
                ERR_NO_EXTERN => panic!("extern {} not registered", self.handle),
                e => Err(anyhow!(
                    "cgo extern 'get_beacon_randomness' failed with error code {}",
                    e
                )),
            }
        }
    }
}

impl Externs for CgoExterns {}
