use cid::Cid;
use fvm_shared::{
    address::Address,
    encoding::{from_slice, Cbor},
    error::ExitCode,
};

use crate::kernel::{ClassifyResult as _, Context as _, Result};

pub trait Memory {
    fn len(&self) -> usize;
    fn try_slice_mut(&mut self, offset: u32, len: u32) -> Result<&mut [u8]>;
    fn try_slice(&self, offset: u32, len: u32) -> Result<&[u8]>;

    fn read_cid(&self, offset: u32) -> Result<Cid> {
        // TODO: max CID length
        Cid::read_bytes(self.try_slice(offset, self.len() as u32)?)
            .or_error(ExitCode::SysErrIllegalArgument)
            .context("failed to parse CID")
    }

    fn read_address(&self, offset: u32, len: u32) -> Result<Address> {
        let bytes = self.try_slice(offset, len)?;
        Address::from_bytes(bytes).or_error(ExitCode::SysErrIllegalArgument)
    }

    fn read_cbor<T: Cbor>(&self, offset: u32, len: u32) -> Result<T> {
        let bytes = self.try_slice(offset, len)?;
        from_slice(bytes).or_error(ExitCode::SysErrIllegalArgument)
    }
}

impl Memory for [u8] {
    fn len(&self) -> usize {
        <[u8]>::len(self)
    }
    fn try_slice(&self, offset: u32, len: u32) -> Result<&[u8]> {
        self.get(offset as usize..)
            .and_then(|data| data.get(..len as usize))
            .ok_or_else(|| format!("buffer {} (length {}) out of bounds", offset, len))
            .or_error(ExitCode::SysErrIllegalArgument)
    }
    fn try_slice_mut(&mut self, offset: u32, len: u32) -> Result<&mut [u8]> {
        self.get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len as usize))
            .ok_or_else(|| format!("buffer {} (length {}) out of bounds", offset, len))
            .or_error(ExitCode::SysErrIllegalArgument)
    }
}
