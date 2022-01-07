use std::ops::{Deref, DerefMut};

use cid::Cid;
use fvm_shared::{
    address::Address,
    encoding::{from_slice, Cbor},
    error::ExitCode,
};

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
    pub fn new<'a>(m: &'a mut [u8]) -> &'a mut Memory {
        unsafe { &mut *(m as *mut [u8] as *mut Memory) }
    }

    pub fn try_slice(&self, offset: u32, len: u32) -> Result<&[u8]> {
        self.get(offset as usize..)
            .and_then(|data| data.get(..len as usize))
            .ok_or_else(|| format!("buffer {} (length {}) out of bounds", offset, len))
            .or_error(ExitCode::SysErrIllegalArgument)
    }
    pub fn try_slice_mut(&mut self, offset: u32, len: u32) -> Result<&mut [u8]> {
        self.get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len as usize))
            .ok_or_else(|| format!("buffer {} (length {}) out of bounds", offset, len))
            .or_error(ExitCode::SysErrIllegalArgument)
    }

    pub fn read_cid(&self, offset: u32) -> Result<Cid> {
        // TODO: max CID length
        Cid::read_bytes(self.try_slice(offset, self.len() as u32)?)
            .or_error(ExitCode::SysErrIllegalArgument)
            .context("failed to parse CID")
    }

    pub fn read_address(&self, offset: u32, len: u32) -> Result<Address> {
        let bytes = self.try_slice(offset, len)?;
        Address::from_bytes(bytes).or_error(ExitCode::SysErrIllegalArgument)
    }

    pub fn read_cbor<T: Cbor>(&self, offset: u32, len: u32) -> Result<T> {
        let bytes = self.try_slice(offset, len)?;
        from_slice(bytes).or_error(ExitCode::SysErrIllegalArgument)
    }
}
