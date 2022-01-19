use std::ops::{Deref, DerefMut};

use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::encoding::{from_slice, Cbor};
use fvm_shared::error::ErrorNumber;

use crate::kernel::{ClassifyResult, Context as _, Result};
use crate::syscalls::MAX_CID_LEN;

pub struct Context<'a, K> {
    pub kernel: &'a mut K,
    pub memory: &'a mut Memory,
}

impl<'a, K> Context<'a, K> {
    /// Reborrow the context with a shorter lifetime. Unfortunately, our pointers are internal so we
    /// can't use rust's normal re-borrowing logic.
    pub fn reborrow(&mut self) -> Context<K> {
        Context {
            kernel: self.kernel,
            memory: self.memory,
        }
    }
}

impl<'a, K> TryFrom<&'a mut wasmtime::Caller<'_, K>> for Context<'a, K> {
    type Error = wasmtime::Trap;

    fn try_from(caller: &'a mut wasmtime::Caller<'_, K>) -> std::result::Result<Self, Self::Error> {
        let (memory, kernel) = caller
            .get_export("memory")
            .and_then(|m| m.into_memory())
            .ok_or_else(|| wasmtime::Trap::new("failed to lookup actor memory"))?
            .data_and_store_mut(caller);
        Ok(Context {
            kernel,
            memory: Memory::new(memory),
        })
    }
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
        Cid::read_bytes(self.try_slice(offset, MAX_CID_LEN as u32)?)
            .or_error(ErrorNumber::IllegalArgument)
            .context("failed to parse CID")
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
