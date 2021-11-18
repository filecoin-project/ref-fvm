use anyhow;
use cid::{self, Cid};
use wasmtime::{self, Caller, Engine, Linker, Trap};

mod runtime;

pub use runtime::{Config, DefaultRuntime, Runtime};

// Computes the encoded size of a varint.
// TODO: move this to the varint crate.
fn uvarint_size(num: u64) -> u32 {
    let bits = u64::BITS - num.leading_zeros();
    (bits / 7 + (bits % 7 > 0) as u32).min(1) as u32
}

/// Returns the size cid would be, once encoded.
// TODO: move this to the cid/multihash crates.
fn encoded_cid_size(k: &Cid) -> u32 {
    let mh = k.hash();
    let mh_size = uvarint_size(mh.code()) + uvarint_size(mh.size() as u64) + mh.size() as u32;
    match k.version() {
        cid::Version::V0 => mh_size,
        cid::Version::V1 => mh_size + uvarint_size(k.codec()) + 1,
    }
}

mod typestate;

struct Context<'a, R, F = dyn ContextFeatures<Memory = typestate::False>>
where
    F: ContextFeatures + ?Sized,
{
    pub caller: Caller<'a, R>,
    memory: typestate::ConstOption<F::Memory, wasmtime::Memory>,
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

trait ContextFeatures {
    type Memory: typestate::TypeOption<wasmtime::Memory>;
}

impl<'a, R> Context<'a, R, dyn ContextFeatures<Memory = typestate::False>> {
    fn new(caller: Caller<'a, R>) -> Self {
        Context {
            caller,
            memory: typestate::ConstOption::none(),
        }
    }
}

impl<'a, R, F> Context<'a, R, F>
where
    F: ContextFeatures + ?Sized,
{
    fn with_memory(
        mut self,
    ) -> Result<Context<'a, R, dyn ContextFeatures<Memory = typestate::True>>, Trap> {
        // This check is known at compile time and should get optimized out.
        if let Some(memory) = self.memory.into_option() {
            return Ok(Context {
                caller: self.caller,
                memory: typestate::ConstOption::some(memory),
            });
        }
        let mem = self
            .caller
            .get_export("memory")
            .and_then(|m| m.into_memory())
            .ok_or_else(|| Trap::new("failed to lookup actor memory"))?;

        Ok(Context {
            caller: self.caller,
            memory: typestate::ConstOption::some(mem),
        })
    }
}

impl<'a, R, F> Context<'a, R, F>
where
    F: ContextFeatures<Memory = typestate::True> + ?Sized,
{
    fn try_slice_mut(&mut self, offset: u32, len: u32) -> Result<&mut [u8], Trap> {
        self.memory
            .data_mut(&mut self.caller)
            .get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len as usize))
            .ok_or_else(|| Trap::new(format!("buffer {} (length {}) out of bounds", offset, len)))
    }

    fn try_slice(&self, offset: u32, len: u32) -> Result<&[u8], Trap> {
        self.memory
            .data(&self.caller)
            .get(offset as usize..)
            .and_then(|data| data.get(..len as usize))
            .ok_or_else(|| Trap::new(format!("buffer {} (length {}) out of bounds", offset, len)))
    }

    // TODO: this shouldn't require a mutable self.
    // But loading the memory requires that, specically for "get_export".
    fn read_cid(&self, offset: u32) -> Result<Cid, Trap> {
        let memory = self
            .memory
            .data(&self.caller)
            .get(offset as usize..)
            .ok_or_else(|| Trap::new(format!("buffer {} out of bounds", offset)))?;
        Cid::read_bytes(&*memory).map_err(|err| Trap::new(format!("failed to parse CID: {}", err)))
    }
}

fn get_root(caller: Caller<'_, impl Runtime>, cid: u32, cid_max_len: u32) -> Result<u32, Trap> {
    let ctx = Context::new(caller);

    let root = *ctx.data().root();
    let size = encoded_cid_size(&root);
    if cid == 0 || size > cid_max_len {
        return Ok(size);
    }

    let mut ctx = ctx.with_memory()?;
    let mut out_slice = ctx.try_slice_mut(cid, cid_max_len)?;

    root.write_bytes(&mut out_slice)
        .map_err(|err| Trap::new(err.to_string()))?;

    Ok(size)
}

fn set_root(caller: Caller<'_, impl Runtime>, cid: u32) -> Result<(), Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let cid = ctx.read_cid(cid)?;
    ctx.data_mut().set_root(cid);
    // TODO: make sure the new root is reachable.
    Ok(())
}

pub fn environment<R>(engine: &Engine) -> anyhow::Result<Linker<R>>
where
    R: Runtime + 'static, // TODO: get rid of the static, if possible.
{
    let mut linker = Linker::new(engine);
    linker.func_wrap("ipld", "get_root", get_root)?;
    linker.func_wrap("ipld", "set_root", set_root)?;
    Ok(linker)
}
