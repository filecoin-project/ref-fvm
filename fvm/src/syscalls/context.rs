use std::ops::{Deref, DerefMut};

use cid::Cid;
use fvm_ipld_encoding::{from_slice, Cbor};
use fvm_shared::address::Address;
use fvm_shared::error::ErrorNumber;

use crate::kernel::{ClassifyResult, Context as _, Result};

pub struct Context<'a, K> {
    pub kernel: &'a mut K,
    pub memory: &'a mut Memory,
}

#[repr(transparent)]
pub struct Memory([u8]);

impl Deref for Memory {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Memory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Memory {
    #[allow(clippy::needless_lifetimes)]
    pub fn new<'a>(m: &'a mut [u8]) -> &'a mut Memory {
        // We explicitly specify the lifetimes here to ensure that the cast doesn't inadvertently
        // change them.
        unsafe { &mut *(m as *mut [u8] as *mut Memory) }
    }

    pub fn try_slice(&self, offset: u32, len: u32) -> Result<&[u8]> {
        self.get(offset as usize..)
            .and_then(|data| data.get(..len as usize))
            .ok_or_else(|| format!("buffer {} (length {}) out of bounds", offset, len))
            .or_error(ErrorNumber::IllegalArgument)
    }
    pub fn try_slice_mut(&mut self, offset: u32, len: u32) -> Result<&mut [u8]> {
        self.get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len as usize))
            .ok_or_else(|| format!("buffer {} (length {}) out of bounds", offset, len))
            .or_error(ErrorNumber::IllegalArgument)
    }

    pub fn read_cid(&self, offset: u32) -> Result<Cid> {
        // NOTE: Be very careful when changing this code.
        //
        // We intentionally read the CID till the end of memory. We intentionally do not "slice"
        // with a fixed end.
        // - We _can't_ slice MAX_CID_LEN because there may not be MAX_CID_LEN addressable memory
        //   after the offset.
        // - We can safely read from an "arbitrary" sized slice because `Cid::read_bytes` will never
        //   read more than 4 u64 varints and 64 bytes of digest.
        Cid::read_bytes(
            self.0
                .get(offset as usize..)
                .ok_or_else(|| format!("cid at offset {} is out of bounds", offset))
                .or_error(ErrorNumber::IllegalArgument)?,
        )
        .or_error(ErrorNumber::IllegalArgument)
        .context("failed to parse cid")
    }

    pub fn read_address(&self, offset: u32, len: u32) -> Result<Address> {
        let bytes = self.try_slice(offset, len)?;
        Address::from_bytes(bytes).or_error(ErrorNumber::IllegalArgument)
    }

    pub fn read_cbor<T: Cbor>(&self, offset: u32, len: u32) -> Result<T> {
        let bytes = self.try_slice(offset, len)?;
        from_slice(bytes).or_error(ErrorNumber::IllegalArgument)
    }
}

#[cfg(test)]
mod test {
    use cid::multihash::MultihashDigest;

    use super::*;

    // TODO: move this somewhere more useful.
    macro_rules! expect_syscall_err {
        ($code:ident, $res:expr) => {
            match $res.expect_err("expected syscall to fail") {
                $crate::kernel::ExecutionError::Syscall($crate::kernel::SyscallError(
                    _,
                    fvm_shared::error::ErrorNumber::$code,
                )) => {}
                $crate::kernel::ExecutionError::Syscall($crate::kernel::SyscallError(
                    msg,
                    code,
                )) => {
                    panic!(
                        "expected {}, got {}: {}",
                        fvm_shared::error::ErrorNumber::$code,
                        code,
                        msg
                    )
                }
                $crate::kernel::ExecutionError::Fatal(err) => {
                    panic!("got unexpected fatal error: {}", err)
                }
                $crate::kernel::ExecutionError::OutOfGas => {
                    panic!("got unexpected out of gas")
                }
            }
        };
    }

    #[test]
    fn test_read_cid() {
        let k = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(b"foo"));
        let mut k_bytes = k.to_bytes();
        let mem = Memory::new(&mut k_bytes);
        let k2 = mem.read_cid(0).expect("failed to read cid");
        assert_eq!(k, k2);
    }

    #[test]
    fn test_read_cid_truncated() {
        let k = Cid::new_v1(0x55, cid::multihash::Code::Sha2_256.digest(b"foo"));
        let mut k_bytes = k.to_bytes();
        let mem = Memory::new(&mut k_bytes[..20]);
        expect_syscall_err!(IllegalArgument, mem.read_cid(0));
    }

    #[test]
    fn test_read_cid_out_of_bounds() {
        let mem = Memory::new(&mut []);
        expect_syscall_err!(IllegalArgument, mem.read_cid(200));
    }

    #[test]
    fn test_read_slice_out_of_bounds() {
        let mem = Memory::new(&mut []);
        expect_syscall_err!(IllegalArgument, mem.try_slice(10, 0));
        expect_syscall_err!(IllegalArgument, mem.try_slice(u32::MAX, 0));
    }

    #[test]
    fn test_read_slice_empty() {
        let mem = Memory::new(&mut []);
        mem.try_slice(0, 0).expect("slice was in bounds");
    }
}
