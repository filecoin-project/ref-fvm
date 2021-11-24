//use anyhow;
use wasmtime::{self, Engine, Linker, Trap};

use crate::{Error, Runtime};

mod context;
mod ipld;
mod typestate;

impl From<Error> for Trap {
    fn from(e: Error) -> Trap {
        Trap::new(e.to_string())
    }
}

pub fn environment<R>(engine: &Engine) -> anyhow::Result<Linker<R>>
where
    R: Runtime + 'static, // TODO: get rid of the static, if possible.
{
    let mut linker = Linker::new(engine);
    linker.func_wrap("ipld", "get_root", ipld::get_root)?;
    linker.func_wrap("ipld", "set_root", ipld::set_root)?;
    linker.func_wrap("ipld", "open", ipld::open)?;
    linker.func_wrap("ipld", "create", ipld::create)?;
    linker.func_wrap("ipld", "read", ipld::read)?;
    linker.func_wrap("ipld", "stat", ipld::stat)?;
    linker.func_wrap("ipld", "cid", ipld::cid)?;
    Ok(linker)
}
