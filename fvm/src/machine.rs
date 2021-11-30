use super::Config;
use crate::invocation::InvocationContainer;
use crate::node::{CircSupplyCalc, LookbackStateGetter, Rand};
use crate::state_tree::StateTree;
use crate::vm::node::NodeRuntime;
use crate::vm::{ActorRuntime, InvocationContainer};
use blockstore::Blockstore;
use cid::Cid;
use fvm_shared::bigint::BigInt;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;
use wasmtime::{Engine, Instance};

pub struct Machine<'a, R: Rand, C: CircSupplyCalc, B: Blockstore, L: LookbackStateGetter<B>> {
    config: Config,
    engine: &'a Engine, // wasmtime engine
    state_tree: StateTree<'a, B>,
    epoch: ChainEpoch,
    base_fee: BigInt,
    network_version_getter: N,

    // providing these in pieces for future testability.
    blockstore: B,
    rand: R,
    circ_supply_calc: C,
    lookback: L,

    // TODO figure out type
    commit_buffer: PhantomData<()>,
    verifier: PhantomData<V>,
    gas_limit: u64, // TODO fix

    call_stack_mgr: &'a CallStackManager,
}

pub struct CallStackManager {
    /// The invocation containers managed by this Engine for the current call stack.
    current: VecDeque<&'a InvocationContainer<'a, AR>>,
}

impl CallStackManager {}

impl<'a, AR> Machine<'a, AR, C, B, L> {
    // TODO add all constructor arguments.
    pub fn new<'a, AR>(
        config: Config,
        epoch: ChainEpoch,
        base_fee: TokenAmount,
        store: B,
        state_root: Cid,
    ) -> Machine<'a, AR, C, B, L> {
        let mut engine = Engine::new(&config.engine)?;
        // TODO initialize the engine
        // TODO instantiate state tree with root and blockstore.
        // TODO bind engine-level store data (NodeInvoker?, Gas charger).
        Machine {
            config,
            epoch,
            base_fee,
            engine: &engine,
            state_tree: StateTree::new_from_root(store, &state_root)?,
            network_version_getter: (),
            blockstore: B,
            rand: (),
            circ_supply_calc: (),
            lookback: (),
            commit_buffer: Default::default(),
            verifier: Default::default(),
            gas_limit: 0,
            call_stack_mgr: &CallStackManager {},
        }
    }

    pub fn engine(&self) -> &Engine {
        self.engine
    }

    pub fn config(&self) -> Config {
        self.config
    }

    pub fn process_message(/*Message*/) -> anyhow::Result<() /*Receipt*/> {}
}
