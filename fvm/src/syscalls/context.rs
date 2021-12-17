use crate::kernel::ExecutionError;
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::encoding::{from_slice, Cbor};
use wasmtime::{Caller, Trap};

use super::typestate;

pub struct Context<'a, R, F = dyn ContextFeatures<Memory = typestate::False>>
where
    F: ContextFeatures + ?Sized,
{
    pub caller: Caller<'a, R>,
    memory: <F::Memory as typestate::Select<wasmtime::Memory, ()>>::Type,
}

impl<'a, R, F> std::ops::Deref for Context<'a, R, F>
where
    F: ContextFeatures + ?Sized,
{
    type Target = Caller<'a, R>;
    fn deref(&self) -> &Self::Target {
        &self.caller
    }
}

impl<'a, R, F> std::ops::DerefMut for Context<'a, R, F>
where
    F: ContextFeatures + ?Sized,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.caller
    }
}

pub trait ContextFeatures {
    type Memory: typestate::Select<wasmtime::Memory, ()>;
}

impl<'a, R> Context<'a, R, dyn ContextFeatures<Memory = typestate::False>> {
    pub fn new(caller: Caller<'a, R>) -> Self {
        Context { caller, memory: () }
    }
}

impl<'a, R, F> Context<'a, R, F>
where
    F: ContextFeatures<Memory = typestate::False> + ?Sized,
{
    pub fn with_memory(
        mut self,
    ) -> Result<Context<'a, R, dyn ContextFeatures<Memory = typestate::True>>, Trap> {
        let mem = self
            .caller
            .get_export("memory")
            .and_then(|m| m.into_memory())
            .ok_or_else(|| Trap::new("failed to lookup actor memory"))?;

        Ok(Context {
            caller: self.caller,
            memory: mem,
        })
    }
}

impl<'a, R, F> Context<'a, R, F>
where
    F: ContextFeatures<Memory = typestate::True> + ?Sized,
{
    pub fn try_slice_mut(&mut self, offset: u32, len: u32) -> Result<&mut [u8], Trap> {
        self.memory
            .data_mut(&mut self.caller)
            .get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len as usize))
            .ok_or_else(|| Trap::new(format!("buffer {} (length {}) out of bounds", offset, len)))
    }

    #[allow(dead_code)]
    pub fn try_slice(&self, offset: u32, len: u32) -> Result<&[u8], Trap> {
        self.memory
            .data(&self.caller)
            .get(offset as usize..)
            .and_then(|data| data.get(..len as usize))
            .ok_or_else(|| Trap::new(format!("buffer {} (length {}) out of bounds", offset, len)))
    }

    // Ew. Let's consider just returning a memory + runtime as separate objects so we don't need to
    // do this.
    pub fn try_slice_and_runtime(
        &mut self,
        offset: u32,
        len: u32,
    ) -> Result<(&mut [u8], &mut R), Trap> {
        let (data, rt) = self.memory.data_and_store_mut(&mut self.caller);
        let slice = data
            .get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len as usize))
            .ok_or_else(|| {
                Trap::new(format!("buffer {} (length {}) out of bounds", offset, len))
            })?;
        Ok((slice, rt))
    }

    pub fn read_cid(&self, offset: u32) -> Result<Cid, Trap> {
        // TODO: max CID length
        let memory = self
            .memory
            .data(&self.caller)
            .get(offset as usize..)
            .ok_or_else(|| Trap::new(format!("buffer {} out of bounds", offset)))?;
        Cid::read_bytes(&*memory).map_err(|err| Trap::new(format!("failed to parse CID: {}", err)))
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
