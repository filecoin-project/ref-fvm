use anyhow::Context as _;
use cid::Cid;
use fvm_shared::{
    address::Address,
    encoding::{from_slice, Cbor},
    error::ExitCode,
};
use wasmtime::Caller;

use crate::kernel::{ClassifyResult as _, Context as _, Result};

pub trait Context {
    type Kernel: crate::kernel::Kernel;
    fn kernel(&mut self) -> &mut Self::Kernel;
    fn kernel_and_memory(&mut self) -> Result<(&mut Self::Kernel, Memory<'_>)>;
}

impl<'a, K> Context for Caller<'a, K>
where
    K: crate::kernel::Kernel,
{
    type Kernel = K;

    fn kernel(&mut self) -> &mut Self::Kernel {
        self.data_mut()
    }

    fn kernel_and_memory(&mut self) -> Result<(&mut Self::Kernel, Memory<'_>)> {
        let (mem, data) = self
            .get_export("memory")
            .and_then(|m| m.into_memory())
            .context("failed to lookup actor memory")
            .or_fatal()?
            .data_and_store_mut(self);
        Ok((data, Memory { memory: mem }))
    }
}

pub struct Memory<'a> {
    memory: &'a mut [u8],
}

impl<'a> Memory<'a> {
    pub fn try_slice_mut(&mut self, offset: u32, len: u32) -> Result<&mut [u8]> {
        self.memory
            .get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len as usize))
            .ok_or_else(|| format!("buffer {} (length {}) out of bounds", offset, len))
            .or_error(ExitCode::SysErrIllegalArgument)
    }

    pub fn try_slice(&self, offset: u32, len: u32) -> Result<&[u8]> {
        self.memory
            .get(offset as usize..)
            .and_then(|data| data.get(..len as usize))
            .ok_or_else(|| format!("buffer {} (length {}) out of bounds", offset, len))
            .or_error(ExitCode::SysErrIllegalArgument)
    }

    pub fn read_cid(&self, offset: u32) -> Result<Cid> {
        // TODO: max CID length
        let memory = self
            .memory
            .get(offset as usize..)
            .ok_or_else(|| format!("buffer {} out of bounds", offset))
            .or_error(ExitCode::SysErrIllegalArgument)?;
        Cid::read_bytes(memory)
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
