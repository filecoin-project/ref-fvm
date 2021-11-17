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
struct Context<'a, R> {
    pub caller: Caller<'a, R>,
    memory: Option<wasmtime::Memory>,
}

impl<'a, R> std::ops::Deref for Context<'a, R> {
    type Target = Caller<'a, R>;
    fn deref(&self) -> &Self::Target {
        &self.caller
    }
}

impl<'a, R> std::ops::DerefMut for Context<'a, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.caller
    }
}

impl<'a, R> Context<'a, R> {
    fn new(caller: Caller<'a, R>) -> Self {
        Context {
            caller,
            memory: None,
        }
    }

    fn load_memory_mut(&mut self) -> Result<&mut [u8], Trap> {
        // TODO: looking up the memory could be slow. Ideally, we'd do it once when instantiating
        // the contract instead of on every syscall.
        Ok(match &mut self.memory {
            Some(memory) => memory,
            mem => mem.insert(
                self.caller
                    .get_export("memory")
                    .and_then(|m| m.into_memory())
                    .ok_or_else(|| Trap::new("failed to lookup actor memory"))?,
            ),
        }
        .data_mut(&mut self.caller))
    }

    fn try_slice_mut(&mut self, offset: u32, len: u32) -> Result<&mut [u8], Trap> {
        self.load_memory_mut()?
            .get_mut(offset as usize..)
            .and_then(|data| data.get_mut(..len as usize))
            .ok_or_else(|| Trap::new(format!("buffer {} (length {}) out of bounds", offset, len)))
    }

    fn read_cid(&mut self, offset: u32) -> Result<Cid, Trap> {
        let mut memory = &*self
            .load_memory_mut()?
            .get_mut(offset as usize..)
            .ok_or_else(|| Trap::new(format!("buffer {} out of bounds", offset)))?;
        Cid::read_bytes(&mut memory)
            .map_err(|err| Trap::new(format!("failed to parse CID: {}", err)))
    }
}

fn get_root(caller: Caller<'_, impl Runtime>, cid: u32, cid_max_len: u32) -> Result<u32, Trap> {
    let mut ctx = Context::new(caller);

    let root = *ctx.data().root();
    let size = encoded_cid_size(&root);
    if cid == 0 || size > cid_max_len {
        return Ok(size);
    }

    let mut out_slice = ctx.try_slice_mut(cid, cid_max_len)?;

    root.write_bytes(&mut out_slice)
        .map_err(|err| Trap::new(err.to_string()))?;

    Ok(size)
}

fn set_root(caller: Caller<'_, impl Runtime>, cid: u32) -> Result<(), Trap> {
    let mut ctx = Context::new(caller);
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
