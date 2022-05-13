use anyhow::{anyhow, Context, Result};
use cid::Cid;
use fvm::call_manager::DefaultCallManager;
use fvm::executor::DefaultExecutor;
use fvm::machine::{DefaultMachine, Engine, Machine, NetworkConfig};
use fvm::state_tree::{ActorState, StateTree};
use fvm::{init_actor, system_actor, DefaultKernel};
use fvm_ipld_blockstore::{Block, Blockstore};
use fvm_ipld_encoding::{ser, CborStore};
use fvm_ipld_hamt::Hamt;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use fvm_shared::{ActorID, IPLD_RAW};
use libsecp256k1::{PublicKey, SecretKey};
use multihash::Code;

use crate::builtin::{
    fetch_builtin_code_cid, import_builtin_actors, set_init_actor, set_sys_actor,
};
use crate::dummy;
use crate::dummy::DummyExterns;
use crate::error::Error::{FailedToFlushTree, NoManifestInformation, NoRootCid};

const DEFAULT_BASE_FEE: u64 = 100;

pub trait Store: Blockstore + Sized + 'static {}

pub type IntegrationExecutor<B> =
    DefaultExecutor<DefaultKernel<DefaultCallManager<DefaultMachine<B, DummyExterns>>>>;

pub type Account = (ActorID, Address);

pub struct Tester<B: Blockstore + 'static> {
    // Network version used in the test
    nv: NetworkVersion,
    // Builtin actors root Cid used in the Machine
    builtin_actors: Cid,
    // Accounts actor cid
    accounts_code_cid: Cid,
    // Custom code cid deployed by developer
    code_cids: Vec<Cid>,
    // Executor used to interact with deployed actors.
    pub executor: Option<IntegrationExecutor<B>>,
    // State tree constructed before instantiating the Machine
    pub state_tree: Option<StateTree<B>>,
}

impl<B> Tester<B>
where
    B: Blockstore,
{
    pub fn new(nv: NetworkVersion, stv: StateTreeVersion, blockstore: B) -> Result<Self> {
        // Load the builtin actors bundles into the blockstore.
        let nv_actors = import_builtin_actors(&blockstore)?;

        // Get the builtin actors index for the concrete network version.
        let builtin_actors = *nv_actors.get(&nv).ok_or(NoRootCid(nv))?;

        let (manifest_version, manifest_data_cid): (u32, Cid) =
            match blockstore.get_cbor(&builtin_actors)? {
                Some((manifest_version, manifest_data)) => (manifest_version, manifest_data),
                None => return Err(NoManifestInformation(builtin_actors).into()),
            };

        // Get sys and init actors code cid
        let (sys_code_cid, init_code_cid, accounts_code_cid) =
            fetch_builtin_code_cid(&blockstore, &manifest_data_cid, manifest_version)?;

        // Initialize state tree
        let mut state_tree = StateTree::new(blockstore, stv).map_err(anyhow::Error::from)?;

        // Insert an empty HAMT.
        let empty_cid = Hamt::<_, String>::new_with_bit_width(state_tree.store(), 5)
            .flush()
            .unwrap();

        // Deploy init and sys actors
        let sys_state = system_actor::State { builtin_actors };
        set_sys_actor(&mut state_tree, sys_state, sys_code_cid)?;

        let init_state = init_actor::State {
            address_map: empty_cid,
            next_id: 100,
            network_name: "test".to_owned(),
        };
        set_init_actor(&mut state_tree, init_code_cid, init_state)?;

        Ok(Tester {
            nv,
            builtin_actors,
            executor: None,
            code_cids: vec![],
            state_tree: Some(state_tree),
            accounts_code_cid,
        })
    }

    /// Creates new accounts in the testing context
    /// Inserts the specified number of accounts in the state tree, all with 1000 FIL，returning their IDs and Addresses.
    pub fn create_accounts<const N: usize>(&mut self) -> Result<[Account; N]> {
        use rand::SeedableRng;

        let rng = &mut rand_chacha::ChaCha8Rng::seed_from_u64(8);

        let mut ret: [Account; N] = [(0, Address::default()); N];
        for account in ret.iter_mut().take(N) {
            let priv_key = SecretKey::random(rng);
            *account = self.make_secp256k1_account(
                priv_key,
                TokenAmount::from(10u8) * TokenAmount::from(1000),
            )?;
        }
        Ok(ret)
    }

    /// Set a new state in the state tree
    pub fn set_state<S: ser::Serialize>(&mut self, state: &S) -> Result<Cid> {
        // Put state in tree
        let state_cid = self
            .state_tree
            .as_mut()
            .unwrap()
            .store()
            .put_cbor(state, Code::Blake2b256)?;

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
        // Register actor address
        self.state_tree
            .as_mut()
            .unwrap()
            .register_new_address(&actor_address)
            .unwrap();

        // Put the WASM code into the blockstore.
        let code_cid = put_wasm_code(self.state_tree.as_mut().unwrap().store(), wasm_bin)?;

        // Add code cid to list of deployed contract
        self.code_cids.push(code_cid);

        // Initialize actor state
        let actor_state = ActorState::new(code_cid, state_cid, balance, 1);

        // Create actor
        self.state_tree
            .as_mut()
            .unwrap()
            .set_actor(&actor_address, actor_state)
            .map_err(anyhow::Error::from)?;

        Ok(())
    }

    /// Sets the Machine and the Executor in our Tester structure.
    pub fn instantiate_machine(&mut self) -> Result<()> {
        // Take the state tree and leave None behind.
        let mut state_tree = self.state_tree.take().unwrap();

        // Calculate the state root.
        let state_root = state_tree
            .flush()
            .map_err(anyhow::Error::from)
            .context(FailedToFlushTree)?;

        // Consume the state tree and take the blockstore.
        let blockstore = state_tree.into_store();

        let mut nc = NetworkConfig::new(self.nv);
        nc.override_actors(self.builtin_actors);

        let mut mc = nc.for_epoch(0, state_root);
        mc.set_base_fee(TokenAmount::from(DEFAULT_BASE_FEE));

        let machine = DefaultMachine::new(
            &Engine::new_default((&mc.network.clone()).into())?,
            &mc,
            blockstore,
            dummy::DummyExterns,
        )?;

        let executor = DefaultExecutor::<DefaultKernel<DefaultCallManager<_>>>::new(machine);
        executor
            .engine()
            .preload(executor.blockstore(), &self.code_cids)?;

        self.executor = Some(executor);

        Ok(())
    }

    /// Get blockstore
    pub fn blockstore(&self) -> &dyn Blockstore {
        if self.executor.is_some() {
            self.executor.as_ref().unwrap().blockstore()
        } else {
            self.state_tree.as_ref().unwrap().store()
        }
    }

    /// Put account with specified private key and balance
    pub fn make_secp256k1_account(
        &mut self,
        priv_key: SecretKey,
        init_balance: TokenAmount,
    ) -> Result<Account> {
        let pub_key = PublicKey::from_secret_key(&priv_key);
        let pub_key_addr = Address::new_secp256k1(&pub_key.serialize())?;

        let state_tree = self
            .state_tree
            .as_mut()
            .ok_or_else(|| anyhow!("unable get state tree"))?;
        let assigned_addr = state_tree.register_new_address(&pub_key_addr).unwrap();
        let state = fvm::account_actor::State {
            address: pub_key_addr,
        };

        let cid = state_tree.store().put_cbor(&state, Code::Blake2b256)?;

        let actor_state = ActorState {
            code: self.accounts_code_cid,
            state: cid,
            sequence: 0,
            balance: init_balance,
        };

        state_tree
            .set_actor(&Address::new_id(assigned_addr), actor_state)
            .map_err(anyhow::Error::from)?;
        Ok((assigned_addr, pub_key_addr))
    }
}
/// Inserts the WASM code for the actor into the blockstore.
fn put_wasm_code(blockstore: &impl Blockstore, wasm_binary: &[u8]) -> Result<Cid> {
    let cid = blockstore.put(
        Code::Blake2b256,
        &Block {
            codec: IPLD_RAW,
            data: wasm_binary,
        },
    )?;
    Ok(cid)
}
