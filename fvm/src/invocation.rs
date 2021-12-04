use anyhow::Result;
use blockstore::Blockstore;
use std::collections::VecDeque;
#[allow(unused_imports)]
use wasmtime::{Config as WasmtimeConfig, Engine, Instance, Linker, Module, Store};

/// An entry in the return stack.
type ReturnEntry = (bool, Vec<u8>);

/// TODO
/// TODO This module needs to be heavily revisited.
/// TODO

#[derive(Default)]
pub struct InvocationContainer<'a> {
    /// The machine to which this invocation container is bound.
    /// TODO likely don't need this reference since the syscall handlers
    /// will have access to the Kernel through store data.
    // machine: &'a Machine<'a, B, E>,
    /// The actor's bytecode.
    actor_bytecode: &'a [u8],
    /// The wasmtime instance this container is running.
    /// TODO might not need this handle in the state.
    instance: &'a Instance,

    /// Stack of return data owned by the invocation container, and made
    /// available to the actor.
    /// TODO If this is necessary; could just return the CID of the result block.
    return_stack: VecDeque<ReturnEntry>,
    ///// TODO gas charger should not be wired here. At this point in time, we are
    ///// charging gas explicitly on syscalls.
    // gas_charger: PhantomData<()>,
}

/// TODO it's possible that the invocation container doesn't need to exist
/// as an object; instead the invocation container could be the "store data"
/// inside the wasmtime store. If so, the CallStack would instantiate the
/// wasmtime::Instance and wire in the store data.
///
/// Although having said that, that solution is entirely wasmtime specific, and
/// will lock us right into that runtime. We probably _should_ have an
/// InvocationContainer to abstract underlying WASM runtime implementation
/// details.
impl<'a> InvocationContainer<'a> {
    fn new<B>(config: &super::Config, wasm_bytecode: &[u8]) -> Result<Self>
    where
        B: Blockstore,
    {
        // /// TODO implement
        // use crate::DefaultKernel;
        // let module = Module::new(&engine, wasm_bytecode)?;
        //
        // // let config = fvm::Config { max_pages: 10 };
        // // let bs = MemoryBlockstore::default();
        // // let root_block = b"test root block";
        // // let root_cid = Cid::new_v1(0x55, MhCode::Sha2_256.digest(root_block));
        // bs.put(&root_cid, root_block)?;
        //
        // let runtime = DefaultKernel::new(blockstore, root_cid);
        //
        // let mut store = Store::new(&engine, runtime);
        todo!()
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
