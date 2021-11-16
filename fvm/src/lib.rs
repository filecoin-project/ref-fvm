use std::sync::{Arc, Mutex, MutexGuard};

use wasmer::{Function, ImportObject, Store, WasmerEnv};

mod runtime;

use runtime::Runtime;

#[derive(WasmerEnv)]
struct Env<R>
where
    R: Send,
{
    runtime: Arc<Mutex<R>>,
    // TODO: needs access to the store.
}

impl<R> Clone for Env<R>
where
    R: Send,
{
    fn clone(&self) -> Self {
        Env {
            runtime: self.runtime.clone(),
        }
    }
}

impl<R> Env<R>
where
    R: Send,
{
    fn lock(&self) -> MutexGuard<R> {
        self.runtime.lock().expect("lock poisoned")
    }
}

// TODO: this really needs an env. BUT, in order to get that to work, we need to handle some lifetime problems...
fn get_root<R>(env: &Env<R>, _cid: i32, _cid_max_len: i32) -> i32
where
    R: Send + Runtime,
{
    //let env = env.lock();
    //env.root()
    panic!("still working on this")
}

// TODO: consider reusing this object if it's a bottleneck.
pub fn environment<R>(rt: Arc<Mutex<R>>, store: &Store) -> ImportObject
where
    R: Runtime + Send + 'static,
{
    let env = Env { runtime: rt };
    //let max_memory = env.lock().config().max_pages;

    let get_root_function = Function::new_native_with_env(store, env, get_root);
    wasmer::imports! {
        "ipld" => {
            "get_root" => get_root_function,
        }
    }
}
