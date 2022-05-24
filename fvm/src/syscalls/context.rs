use std::io::Cursor;
use std::ops::{Deref, DerefMut};
use std::panic;

use cid::Cid;
use fvm_ipld_encoding::{from_slice, Cbor};
use fvm_shared::address::Address;
use fvm_shared::error::ErrorNumber;
use fvm_shared::MAX_CID_LEN;

use crate::kernel::{ClassifyResult, Context as _, Result};
use crate::syscall_error;

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

    pub fn check_bounds(&self, offset: u32, len: u32) -> Result<()> {
        if (offset as u64) + (len as u64) <= (self.0.len() as u64) {
            Ok(())
        } else {
            Err(
                syscall_error!(IllegalArgument; "buffer {} (length {}) out of bounds", offset, len)
                    .into(),
            )
        }
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

    pub fn write_cid(&mut self, k: &Cid, offset: u32, len: u32) -> Result<u32> {
        let out = self.try_slice_mut(offset, len)?;

        let mut buf = Cursor::new([0u8; MAX_CID_LEN]);
        // At the moment, all CIDs are gauranteed to fit in 100 bytes (statically) because the max
        // digest size is 64, the max varint size is 9, and there are 4 varints plus the digest.
        k.write_bytes(&mut buf).expect("failed to format a cid");
        let len = buf.position() as usize;
        if len > out.len() {
            return Err(syscall_error!(BufferTooSmall; "cid output buffer is too small").into());
        }
        out[..len].copy_from_slice(&buf.get_ref()[..len]);
        Ok(len as u32)
    }

    pub fn read_address(&self, offset: u32, len: u32) -> Result<Address> {
        let bytes = self.try_slice(offset, len)?;
        Address::from_bytes(bytes).or_error(ErrorNumber::IllegalArgument)
    }

    pub fn read_cbor<T: Cbor>(&self, offset: u32, len: u32) -> Result<T> {
        let bytes = self.try_slice(offset, len)?;
        // Catch panics when decoding cbor from actors, _just_ in case.
        match panic::catch_unwind(|| from_slice(bytes).or_error(ErrorNumber::IllegalArgument)) {
            Ok(v) => v,
            Err(e) => {
                log::error!("panic when decoding cbor from actor: {:?}", e);
                Err(syscall_error!(IllegalArgument; "panic when decoding cbor from actor").into())
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const RAW: u64 = 0x55;
    const SHA2_256: u64 = 0x12;
    const HASH: &[u8] = b"\x2C\x26\xB4\x6B\x68\xFF\xC6\x8F\xF9\x9B\x45\x3C\x1D\x30\x41\x34\x13\x42\x2D\x70\x64\x83\xBF\xA0\xF9\x8A\x5E\x88\x62\x66\xE7\xAE";

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
        let hash = cid::multihash::Multihash::wrap(SHA2_256, HASH).unwrap();
        let k = Cid::new_v1(RAW, hash);
        let mut k_bytes = k.to_bytes();
        let mem = Memory::new(&mut k_bytes);
        let k2 = mem.read_cid(0).expect("failed to read cid");
        assert_eq!(k, k2);
    }

    #[test]
    fn test_read_cid_truncated() {
        let hash = cid::multihash::Multihash::wrap(SHA2_256, HASH).unwrap();
        let k = Cid::new_v1(RAW, hash);
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
