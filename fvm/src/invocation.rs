use super::{ActorRuntime, Machine, Runtime};
use crate::syscalls::bind_syscalls;
use crate::DefaultRuntime;
use anyhow::Result;
use blockstore::Blockstore;
use cid::Cid;
use std::collections::VecDeque;
use std::marker::PhantomData;
use wasmtime::{Config as WasmtimeConfig, Engine, Instance, Linker, Module, Store};

/// An entry in the return stack.
type ReturnEntry = (bool, Vec<u8>);

#[derive(Default)]
pub struct InvocationContainer<'a, AR: ActorRuntime> {
    /// The machine to which this invocation container is bound.
    machine: &'a Machine<'a, AR, NR>,
    /// The actor runtime that processes syscalls from this actor.
    actor_runtime: AR,
    /// The actor's bytecode.
    actor_bytecode: &'a [u8],
    /// The wasmtime instance this container is running.
    instance: &'a Instance,

    /// Stack of return data owned by the invocation container, and made
    /// available to the actor.
    return_stack: VecDeque<ReturnEntry>,

    /// Gas charger is the
    gas_charger: PhantomData<()>,
}

impl InvocationContainer<R> {
    fn new<B>(config: &super::Config, blockstore: B, wasm_bytecode: &[u8]) -> Result<Self>
    where
        B: Blockstore,
    {
        let mut engine = Engine::new(&config.engine)?;
        let module = Module::new(&engine, wasm_bytecode)?;

        // let config = fvm::Config { max_pages: 10 };
        // let bs = MemoryBlockstore::default();
        // let root_block = b"test root block";
        // let root_cid = Cid::new_v1(0x55, MhCode::Sha2_256.digest(root_block));
        bs.put(&root_cid, root_block)?;

        let mut linker = Linker::new(&engine);
        bind_syscalls(linker)?;

        let runtime = Runtime::new(blockstore, root_cid);

        let mut linker = fvm::environment(&mut engine)?;
        let mut store = Store::new(&engine, runtime);
    }

    /// Describes the top element in the return stack.
    /// -1 means error, 0 means non-existent, otherwise the length is returned.
    pub fn return_desc(&self) -> u64 {
        self.return_stack.back().map_or(0, |e| {
            if !e.0 {
                return -1;
            }
            e.1.len() as u64
        })
    }

    pub fn return_discard(&mut self) {
        self.return_stack.pop_back();
    }

    /// Copies the top of the stack into
    pub fn return_pop(&mut self, into: &[u8]) {
        self.return_stack.pop_back();
    }
}
