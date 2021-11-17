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

fn get_root(mut caller: Caller<'_, impl Runtime>, cid: u32, cid_max_len: u32) -> Result<u32, Trap> {
    let root = *caller.data().root();
    let size = encoded_cid_size(&root);
    if cid == 0 || size > cid_max_len {
        return Ok(size);
    }

    // TODO: could be slow? Ideally we'd memoize this somehow.
    let memory = caller
        .get_export("memory")
        .and_then(|m| m.into_memory())
        .ok_or_else(|| Trap::new("failed to lookup actor memory"))?;

    let mut out_slice = memory
        .data_mut(&mut caller)
        .get_mut(cid as usize..)
        .and_then(|data| data.get_mut(..cid_max_len as usize))
        .ok_or_else(|| {
            Trap::new(format!(
                "cid buffer {} (length {}) out of bounds",
                cid, cid_max_len
            ))
        })?;

    root.write_bytes(&mut out_slice)
        .map_err(|err| Trap::new(err.to_string()))?;

    Ok(size)
}

pub fn environment<R>(engine: &Engine) -> anyhow::Result<Linker<R>>
where
    R: Runtime + 'static, // TODO: get rid of the static, if possible.
{
    let mut linker = Linker::new(engine);
    linker.func_wrap("ipld", "get_root", get_root)?;
    Ok(linker)
}
