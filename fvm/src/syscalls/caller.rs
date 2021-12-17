use cid::Cid;
use fvm_shared::{
    address::Address,
    encoding::{from_slice, Cbor},
};
use wasmtime::{Caller, Trap};

use crate::kernel::ExecutionError;

pub fn get_kernel<'a, 'b, K>(mut caller: &'a mut Caller<'b, K>) -> &'a mut K {
    caller.data_mut()
}

pub fn get_kernel_and_memory<'a, 'b, K>(
    caller: &'a mut Caller<'b, K>,
) -> Result<(&'a mut K, Memory<'a>), Trap> {
    let (mem, data) = caller
        .get_export("memory")
        .and_then(|m| m.into_memory())
        .ok_or_else(|| Trap::new("failed to lookup actor memory"))?
        .data_and_store_mut(caller);
    Ok((data, Memory { memory: mem }))
}

pub struct Memory<'a> {
    memory: &'a mut [u8],
}

impl<'a> Memory<'a> {
    pub fn try_slice_mut(&mut self, offset: u32, len: u32) -> Result<&mut [u8], Trap> {
        self.memory
            .get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len as usize))
            .ok_or_else(|| Trap::new(format!("buffer {} (length {}) out of bounds", offset, len)))
    }

    pub fn try_slice(&self, offset: u32, len: u32) -> Result<&[u8], Trap> {
        self.memory
            .get(offset as usize..)
            .and_then(|data| data.get(..len as usize))
            .ok_or_else(|| Trap::new(format!("buffer {} (length {}) out of bounds", offset, len)))
    }

    pub fn read_cid(&self, offset: u32) -> Result<Cid, Trap> {
        // TODO: max CID length
        let memory = self
            .memory
            .get(offset as usize..)
            .ok_or_else(|| Trap::new(format!("buffer {} out of bounds", offset)))?;
        Cid::read_bytes(memory).map_err(|err| Trap::new(format!("failed to parse CID: {}", err)))
    }

    pub fn read_address(&self, offset: u32, len: u32) -> Result<Address, Trap> {
        let bytes = self.try_slice(offset, len)?;
        Address::from_bytes(bytes)
            .map_err(ExecutionError::from)
            .map_err(Trap::from)
    }

    pub fn read_cbor<T: Cbor>(&self, offset: u32, len: u32) -> Result<T, Trap> {
        let bytes = self.try_slice(offset, len)?;
        from_slice(bytes)
            .map_err(ExecutionError::from)
            .map_err(Trap::from)
    }
}
