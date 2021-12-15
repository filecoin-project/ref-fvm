extern "C" {
    pub fn cgobs_get(
        store: i32,
        k: *const u8,
        k_len: i32,
        block: *mut *mut u8,
        size: *mut i32,
    ) -> i32;
    pub fn cgobs_put(store: i32, k: *const u8, k_len: i32, block: *const u8, block_len: i32)
        -> i32;
    pub fn cgobs_delete(store: i32, k: *const u8, k_len: i32) -> i32;
    pub fn cgobs_has(store: i32, k: *const u8, k_len: i32) -> i32;
}

use cid::Cid;
use std::ptr;

use std::error;
use std::fmt;

use super::Blockstore;

const ERR_NO_STORE: i32 = -1;
const ERR_NOT_FOUND: i32 = -2;

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    NotFound,
    Other,
}

pub struct CgoBlockstore {
    handle: i32,
}

impl CgoBlockstore {
    /// Construct a new blockstore from a handle.
    pub fn new(handle: i32) -> CgoBlockstore {
        CgoBlockstore { handle }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("not found"),
            Self::Other => f.write_str("other"),
        }
    }
}

impl error::Error for Error {}

// TODO: Implement a trait. Unfortunately, the chainsafe one is a bit tangled with the concept of a
// datastore.
impl Blockstore for CgoBlockstore {
    type Error = Error;

    fn has(&self, k: &Cid) -> Result<bool, Self::Error> {
        let k_bytes = k.to_bytes();
        unsafe {
            match cgobs_has(self.handle, k_bytes.as_ptr(), k_bytes.len() as i32) {
                // We shouldn't get an "error not found" here, but there's no reason to be strict
                // about it.
                0 | ERR_NOT_FOUND => Ok(false),
                1 => Ok(true),
                // Panic on unknown values. There's a bug in the program.
                r @ 2.. => panic!("invalid return value from has: {}", r),
                // Panic if the store isn't registered. This means something _very_ unsafe is going
                // on and there is a bug in the program.
                ERR_NO_STORE => panic!("blockstore {} not registered", self.handle),
                // Otherwise, return "other". We should add error codes in the future.
                _ => Err(Error::Other),
            }
        }
    }

    fn get(&self, k: &Cid) -> Result<Option<Vec<u8>>, Self::Error> {
        let k_bytes = k.to_bytes();
        unsafe {
            let mut buf: *mut u8 = ptr::null_mut();
            let mut size: i32 = 0;
            match cgobs_get(
                self.handle,
                k_bytes.as_ptr(),
                k_bytes.len() as i32,
                &mut buf,
                &mut size,
            ) {
                0 => Ok(Some(Vec::from_raw_parts(buf, size as usize, size as usize))),
                r @ 1.. => panic!("invalid return value from has: {}", r),
                ERR_NO_STORE => panic!("blockstore {} not registered", self.handle),
                ERR_NOT_FOUND => Err(Error::NotFound),
                _ => Err(Error::Other),
            }
        }
    }

    fn put_keyed(&self, k: &Cid, block: &[u8]) -> Result<(), Error> {
        let k_bytes = k.to_bytes();
        unsafe {
            match cgobs_put(
                self.handle,
                k_bytes.as_ptr(),
                k_bytes.len() as i32,
                block.as_ptr(),
                block.len() as i32,
            ) {
                0 => Ok(()),
                r @ 1.. => panic!("invalid return value from has: {}", r),
                ERR_NO_STORE => panic!("blockstore {} not registered", self.handle),
                // This error makes no sense.
                ERR_NOT_FOUND => panic!("not found error on put"),
                _ => Err(Error::Other),
            }
        }
    }
}
