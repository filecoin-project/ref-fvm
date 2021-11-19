use anyhow;
use cid::{self, Cid};
use wasmtime::{self, Caller, Engine, Linker, Trap};

mod runtime;

pub use runtime::{Config, DefaultRuntime, Runtime};

impl From<runtime::Error> for Trap {
    fn from(e: runtime::Error) -> Trap {
        Trap::new(e.to_string())
    }
}

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

trait ContextFeatures {
    type Memory: typestate::Select<wasmtime::Memory, ()>;
}

impl<'a, R> Context<'a, R, dyn ContextFeatures<Memory = typestate::False>> {
    fn new(caller: Caller<'a, R>) -> Self {
        Context { caller, memory: () }
    }
}

impl<'a, R, F> Context<'a, R, F>
where
    F: ContextFeatures<Memory = typestate::False> + ?Sized,
{
    fn with_memory(
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

    // Ew. Let's consider just returning a memory + runtime as separate objects so we don't need to
    // do this.
    fn try_slice_and_runtime(
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

    fn read_cid(&self, offset: u32) -> Result<Cid, Trap> {
        // TODO: max CID length
        let memory = self
            .memory
            .data(&self.caller)
            .get(offset as usize..)
            .ok_or_else(|| Trap::new(format!("buffer {} out of bounds", offset)))?;
        Cid::read_bytes(&*memory).map_err(|err| Trap::new(format!("failed to parse CID: {}", err)))
    }
}

fn get_root(caller: Caller<'_, impl Runtime>, cid_off: u32, cid_len: u32) -> Result<u32, Trap> {
    let ctx = Context::new(caller);

    let root = *ctx.data().root();
    let size = encoded_cid_size(&root);
    if size > cid_len {
        return Ok(size);
    }

    let mut ctx = ctx.with_memory()?;
    let mut out_slice = ctx.try_slice_mut(cid_off, cid_len)?;

    root.write_bytes(&mut out_slice)
        .map_err(|err| Trap::new(err.to_string()))?;

    Ok(size)
}

fn set_root(caller: Caller<'_, impl Runtime>, cid: u32) -> Result<(), Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let cid = ctx.read_cid(cid)?;
    ctx.data_mut().set_root(cid)?;
    Ok(())
}

fn ipld_open(caller: Caller<'_, impl Runtime>, cid: u32) -> Result<u32, Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let cid = ctx.read_cid(cid)?;
    Ok(ctx.data_mut().block_open(&cid)?)
}

fn ipld_create(
    caller: Caller<'_, impl Runtime>,
    codec: u64,
    data_off: u32,
    data_len: u32,
) -> Result<u32, Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let (data, rt) = ctx.try_slice_and_runtime(data_off, data_len)?;
    Ok(rt.block_create(codec, data)?)
}

fn ipld_cid(
    caller: Caller<'_, impl Runtime>,
    id: u32,
    hash_fun: u64,
    hash_len: u32,
    cid_off: u32,
    cid_len: u32,
) -> Result<u32, Trap> {
    let mut ctx = Context::new(caller);
    let cid = ctx.data_mut().block_cid(id, hash_fun, hash_len)?;

    let size = encoded_cid_size(&cid);
    if size > cid_len {
        return Ok(size);
    }

    let mut ctx = ctx.with_memory()?;
    let mut out_slice = ctx.try_slice_mut(cid_off, cid_len)?;

    cid.write_bytes(&mut out_slice)
        .map_err(|err| Trap::new(err.to_string()))?;
    Ok(size)
}

fn ipld_read(
    caller: Caller<'_, impl Runtime>,
    id: u32,
    offset: u32,
    obuf_off: u32,
    obuf_len: u32,
) -> Result<u32, Trap> {
    let mut ctx = Context::new(caller).with_memory()?;
    let (data, rt) = ctx.try_slice_and_runtime(obuf_off, obuf_len)?;
    Ok(rt.block_read(id, offset, data)?)
}

fn ipld_stat(caller: Caller<'_, impl Runtime>, id: u32) -> Result<(u64, u32), Trap> {
    let ctx = Context::new(caller);
    Ok(ctx
        .data()
        .block_stat(id)
        .map(|stat| (stat.codec, stat.size))?)
}

pub fn environment<R>(engine: &Engine) -> anyhow::Result<Linker<R>>
where
    R: Runtime + 'static, // TODO: get rid of the static, if possible.
{
    let mut linker = Linker::new(engine);
    linker.func_wrap("ipld", "get_root", get_root)?;
    linker.func_wrap("ipld", "set_root", set_root)?;
    linker.func_wrap("ipld", "open", ipld_open)?;
    linker.func_wrap("ipld", "create", ipld_create)?;
    linker.func_wrap("ipld", "read", ipld_read)?;
    linker.func_wrap("ipld", "stat", ipld_stat)?;
    linker.func_wrap("ipld", "cid", ipld_cid)?;
    Ok(linker)
}
