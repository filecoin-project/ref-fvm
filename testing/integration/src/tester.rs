use anyhow::{Context, Result};
use cid::Cid;
use fvm::call_manager::DefaultCallManager;
use fvm::executor::DefaultExecutor;
use fvm::machine::{DefaultMachine, Engine};
use fvm::state_tree::{ActorState, StateTree};
use fvm::{init_actor, system_actor, Config, DefaultKernel};
use fvm_ipld_hamt::Hamt;
use fvm_shared::address::Address;
use fvm_shared::bigint::BigInt;
use fvm_shared::blockstore::{Block, Blockstore, CborStore, MemoryBlockstore};
use fvm_shared::econ::TokenAmount;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use fvm_shared::{ActorID, IPLD_RAW};
use multihash::Code;
use num_traits::Zero;

use crate::builtin::{
    fetch_sys_init_code_cid, import_builtin_actors, set_init_actor, set_sys_actor,
};
use crate::dummy;
use crate::dummy::DummyExterns;
use crate::error::Error::{FailedToFlushTree, FailedToLoadCacheConfig, NoRootCid};

const DEFAULT_BASE_FEE: u64 = 100;

pub struct Tester {
    // Network version used in the test
    nv: NetworkVersion,
    // StateTree version used in the test
    stv: StateTreeVersion,
    // Builtin actors root Cid used in the Machine
    builtin_actors: Cid,
    // Blockstore used to instantiate the machine before running executions
    pub initial_blockstore: MemoryBlockstore,
    // Initial state tree cid used to instantiate the machine before running executions
    pub initial_state_root: Cid,
    // Accounts available to interect with the executor
    pub accounts: Vec<(ActorID, Address)>,
    // Executor used to interact with deployed actors.
    pub executor: Option<
        DefaultExecutor<
            DefaultKernel<DefaultCallManager<DefaultMachine<MemoryBlockstore, DummyExterns>>>,
        >,
    >,
}

impl Tester {
    pub fn new(nv: NetworkVersion, stv: StateTreeVersion, accounts_count: usize) -> Result<Self> {
        // Initialize blockstore
        let blockstore = MemoryBlockstore::default();

        // Load the builtin actors bundles into the blockstore.
        let nv_actors = import_builtin_actors(&blockstore)?;

        // Get the builtin actors index for the concrete network version.
        let builtin_actors = *nv_actors.get(&nv).ok_or(NoRootCid(nv))?;

        // Get sys and init actors code cid
        let (sys_code_cid, init_code_cid) =
            fetch_sys_init_code_cid(&blockstore, &builtin_actors, 0)?;

        // Initialize state tree
        let mut state_tree =
            StateTree::new(blockstore, stv.clone()).map_err(anyhow::Error::from)?;

        // Insert an empty HAMT.
        let empty_cid = Hamt::<_, String>::new_with_bit_width(state_tree.store(), 5)
            .flush()
            .unwrap();

        // Deploy init and sys actors
        let sys_state = system_actor::State { builtin_actors };
        set_sys_actor(&mut state_tree, sys_state, sys_code_cid)?;

        let init_state = init_actor::State {
            address_map: empty_cid.clone(),
            next_id: 100,
            network_name: "test".to_owned(),
        };
        set_init_actor(&mut state_tree, init_code_cid, init_state)?;

        // Create 10 accounts.
        let accounts = put_secp256k1_accounts(&mut state_tree, accounts_count)?;

        // Flush initial tree
        let state_root = state_tree
            .flush()
            .map_err(anyhow::Error::from)
            .context(FailedToFlushTree)?;

        let blockstore = state_tree.consume();

        Ok(Tester {
            nv,
            stv,
            builtin_actors,
            accounts,
            initial_blockstore: blockstore,
            initial_state_root: state_root,
            executor: None,
        })
    }

    /// Set a new state in the state tree
    pub fn set_state<S: fvm_shared::encoding::ser::Serialize>(&mut self, state: &S) -> Result<Cid> {
        let mut state_tree = StateTree::new(self.initial_blockstore.clone(), self.stv)
            .map_err(anyhow::Error::from)?;

        let state_cid = state_tree.store().put_cbor(state, Code::Blake2b256)?;

        // Flush tree
        let state_root = state_tree
            .flush()
            .map_err(anyhow::Error::from)
            .context(FailedToFlushTree)?;

        let blockstore = state_tree.consume();

        self.initial_state_root = state_root;
        self.initial_blockstore = blockstore;

        Ok(state_cid)
    }

    /// Set a new at a given address, provided with a given token balance
    pub fn set_actor_from_bin(
        &mut self,
        wasm_bin: &[u8],
        state_cid: Cid,
        actor_address: Address,
        balance: TokenAmount,
    ) -> Result<()> {
        let mut state_tree = StateTree::new(self.initial_blockstore.clone(), self.stv)
            .map_err(anyhow::Error::from)?;
        // Put the WASM code into the blockstore.
        let code_cid = put_wasm_code(state_tree.store(), wasm_bin)?;

        let actor_state = ActorState::new(code_cid, state_cid, balance, 1);

        // Create actor
        state_tree
            .set_actor(&actor_address, actor_state)
            .map_err(anyhow::Error::from)?;

        // Flush tree
        let state_root = state_tree
            .flush()
            .map_err(anyhow::Error::from)
            .context(FailedToFlushTree)?;

        let blockstore = state_tree.consume();

        self.initial_state_root = state_root;
        self.initial_blockstore = blockstore;

        Ok(())
    }

    /// Sets the Machine and the Executor in our Tester structure.
    pub fn instantiate_machine(&mut self) -> Result<()> {
        let mut wasm_conf = wasmtime::Config::default();
        wasm_conf
            .cache_config_load_default()
            .context(FailedToLoadCacheConfig)?;

        let machine = DefaultMachine::new(
            Config {
                max_call_depth: 4096,
                initial_pages: 0,
                max_pages: 1024,
                debug: true, // Enable debug mode by default.
            },
            Engine::default(),
            0,
            BigInt::from(DEFAULT_BASE_FEE),
            BigInt::zero(),
            self.nv,
            self.initial_state_root,
            (0, Some(self.builtin_actors)),
            self.initial_blockstore.clone(),
            dummy::DummyExterns,
        )?;

        self.executor = Some(DefaultExecutor::<DefaultKernel<DefaultCallManager<_>>>::new(machine));

        Ok(())
    }
}
/// Inserts the specified number of accounts in the state tree, all with 1000 FIL,
/// returning their IDs and Addresses.
fn put_secp256k1_accounts(
    state_tree: &mut StateTree<impl Blockstore>,
    count: usize,
) -> Result<Vec<(ActorID, Address)>> {
    use libsecp256k1::{PublicKey, SecretKey};
    use rand::SeedableRng;

    let rng = &mut rand_chacha::ChaCha8Rng::seed_from_u64(8);

    let mut ret = Vec::with_capacity(count);
    for _ in 0..count {
        let priv_key = SecretKey::random(rng);
        let pub_key = PublicKey::from_secret_key(&priv_key);
        let pub_key_addr = Address::new_secp256k1(&pub_key.serialize())?;
        let assigned_addr = state_tree.register_new_address(&pub_key_addr).unwrap();

        ret.push((assigned_addr, pub_key_addr));
    }
    Ok(ret)
}

/// Inserts the WASM code for the actor into the blockstore.
fn put_wasm_code(blockstore: &MemoryBlockstore, wasm_binary: &[u8]) -> Result<Cid> {
    let cid = blockstore.put(
        Code::Blake2b256,
        &Block {
            codec: IPLD_RAW,
            data: wasm_binary,
        },
    )?;
    Ok(cid)
}
