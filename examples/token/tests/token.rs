#![cfg(test)]

use std::error::Error;

use cid::multihash::Code;
use cid::Cid;
use fvm::builtin::{ACCOUNT_ACTOR_CODE_ID, INIT_ACTOR_CODE_ID};
use fvm::call_manager::DefaultCallManager;
use fvm::executor::{ApplyKind, DefaultExecutor, Executor};
use fvm::externs::cgo::CgoExterns;
use fvm::init_actor::INIT_ACTOR_ADDR;
use fvm::machine::DefaultMachine;
use fvm::state_tree::{ActorState, StateTree};
use fvm::{init_actor, Config, DefaultKernel};
use fvm_shared::address::Address;
use fvm_shared::bigint::bigint_ser::BigIntDe;
use fvm_shared::blockstore::{Block, Blockstore, CborStore, MemoryBlockstore};
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::RawBytes;
use fvm_shared::error::ExitCode;
use fvm_shared::message::Message;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use fvm_shared::ActorID;
use fvm_token_actor::{State, TransferParams};
use ipld_hamt::Hamt;

const IPLD_CODEC_RAW: u64 = 0x55; // TODO temporary
const TOKEN_ACTOR_ID: ActorID = 2000;
const NAME: &str = "FVM mock token";
const SYMBOL: &str = "TOK";

type Result<T> = core::result::Result<T, Box<dyn Error>>;

pub mod wasm {
    include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_wasm_binaries() {
            assert!(!WASM_BINARY.unwrap().is_empty());
            assert!(!WASM_BINARY_BLOATY.unwrap().is_empty());
        }
    }
}

struct SetupRet {
    accounts: Vec<(ActorID, Address)>,
    token_actor_addr: Address,
    state_root: Cid,
    blockstore: MemoryBlockstore,
}

/// Tests that the metadata for the token is accessible.
#[test]
pub fn test_metadata() -> Result<()> {
    let setup = setup()?;

    let mut exec = create_executor(setup.state_root, setup.blockstore);

    // Call the name method.
    let ret = exec.execute_message(
        Message {
            version: 0,
            from: setup.accounts[0].1.clone(),
            to: setup.token_actor_addr.clone(),
            sequence: 0,
            value: Default::default(),
            method_num: 1,
            params: Default::default(),
            gas_limit: 1000000000,
            gas_fee_cap: Default::default(),
            gas_premium: Default::default(),
        },
        ApplyKind::Explicit,
        100,
    )?;

    assert_eq!(ExitCode::Ok, ret.msg_receipt.exit_code);
    assert_eq!(
        NAME.to_owned(),
        ret.msg_receipt.return_data.deserialize::<String>()?
    );

    // Call the symbol method.
    let ret = exec.execute_message(
        Message {
            version: 0,
            from: setup.accounts[0].1.clone(),
            to: setup.token_actor_addr.clone(),
            sequence: 1,
            value: Default::default(),
            method_num: 2,
            params: Default::default(),
            gas_limit: 1000000000,
            gas_fee_cap: Default::default(),
            gas_premium: Default::default(),
        },
        ApplyKind::Explicit,
        100,
    )?;

    assert_eq!(ExitCode::Ok, ret.msg_receipt.exit_code);
    assert_eq!(
        SYMBOL.to_owned(),
        ret.msg_receipt.return_data.deserialize::<String>()?
    );

    Ok(())
}

/// Tests that the pre-seeded accounts all have the right token balance.
#[test]
pub fn get_balances() -> Result<()> {
    let setup = setup()?;

    let mut exec = create_executor(setup.state_root, setup.blockstore);

    for (i, (id, _)) in setup.accounts.iter().enumerate() {
        let ret = exec.execute_message(
            Message {
                version: 0,
                from: setup.accounts[0].1.clone(),
                to: setup.token_actor_addr.clone(),
                sequence: i as u64,
                value: Default::default(),
                method_num: 5,
                params: RawBytes::serialize(Address::new_id(id.clone()))?,
                gas_limit: 1000000000,
                gas_fee_cap: Default::default(),
                gas_premium: Default::default(),
            },
            ApplyKind::Explicit,
            100,
        )?;

        assert_eq!(ExitCode::Ok, ret.msg_receipt.exit_code);
        assert_eq!(
            TokenAmount::from(16000 as u64),
            ret.msg_receipt.return_data.deserialize::<BigIntDe>()?.0
        );
    }

    Ok(())
}

#[test]
pub fn test_transfer_by_id() -> Result<()> {
    let setup = setup()?;

    let mut exec = create_executor(setup.state_root, setup.blockstore);

    let sender = setup.accounts[0].1.clone();
    let recipient = setup.accounts[1].1.clone();

    // Execute transfer.
    let ret = exec.execute_message(
        Message {
            version: 0,
            from: sender.clone(),
            to: setup.token_actor_addr.clone(),
            sequence: 0,
            value: Default::default(),
            method_num: 4,
            params: RawBytes::serialize(TransferParams {
                recipient: recipient.clone(),
                amount: TokenAmount::from(4000 as u64),
            })?,
            gas_limit: 1000000000,
            gas_fee_cap: Default::default(),
            gas_premium: Default::default(),
        },
        ApplyKind::Explicit,
        100,
    )?;

    assert_eq!(ExitCode::Ok, ret.msg_receipt.exit_code);

    // Get balance of sender.
    let ret = exec.execute_message(
        Message {
            version: 0,
            from: sender.clone(),
            to: setup.token_actor_addr.clone(),
            sequence: 1,
            value: Default::default(),
            method_num: 5,
            params: RawBytes::serialize(sender.clone())?,
            gas_limit: 1000000000,
            gas_fee_cap: Default::default(),
            gas_premium: Default::default(),
        },
        ApplyKind::Explicit,
        100,
    )?;

    assert_eq!(ExitCode::Ok, ret.msg_receipt.exit_code);
    // Reduced by 4000.
    assert_eq!(
        TokenAmount::from(12000 as u64),
        ret.msg_receipt.return_data.deserialize::<BigIntDe>()?.0
    );

    // Get balance of sender.
    let ret = exec.execute_message(
        Message {
            version: 0,
            from: sender.clone(),
            to: setup.token_actor_addr.clone(),
            sequence: 2,
            value: Default::default(),
            method_num: 5,
            params: RawBytes::serialize(recipient.clone())?,
            gas_limit: 1000000000,
            gas_fee_cap: Default::default(),
            gas_premium: Default::default(),
        },
        ApplyKind::Explicit,
        100,
    )?;

    assert_eq!(ExitCode::Ok, ret.msg_receipt.exit_code);
    // Increased by 4000.
    assert_eq!(
        TokenAmount::from(20000 as u64),
        ret.msg_receipt.return_data.deserialize::<BigIntDe>()?.0
    );

    Ok(())
}

#[test]
pub fn test_fail_self_transfer() -> Result<()> {
    let setup = setup()?;

    let mut exec = create_executor(setup.state_root, setup.blockstore);

    let sender_recipient = setup.accounts[0].1.clone();

    // Execute transfer.
    let ret = exec.execute_message(
        Message {
            version: 0,
            from: sender_recipient.clone(),
            to: setup.token_actor_addr.clone(),
            sequence: 0,
            value: Default::default(),
            method_num: 4,
            params: RawBytes::serialize(TransferParams {
                recipient: sender_recipient.clone(),
                amount: TokenAmount::from(4000 as u64),
            })?,
            gas_limit: 1000000000,
            gas_fee_cap: Default::default(),
            gas_premium: Default::default(),
        },
        ApplyKind::Explicit,
        100,
    )?;

    assert_eq!(ExitCode::ErrIllegalArgument, ret.msg_receipt.exit_code);

    Ok(())
}

#[test]
pub fn test_fail_transfer_to_non_account_actor() -> Result<()> {
    let setup = setup()?;

    let mut exec = create_executor(setup.state_root, setup.blockstore);

    let sender_recipient = setup.accounts[0].1.clone();

    // Execute transfer; try to send the tokens to the token actor itself.
    let ret = exec.execute_message(
        Message {
            version: 0,
            from: sender_recipient.clone(),
            to: setup.token_actor_addr.clone(),
            sequence: 0,
            value: Default::default(),
            method_num: 4,
            params: RawBytes::serialize(TransferParams {
                recipient: setup.token_actor_addr.clone(),
                amount: TokenAmount::from(4000 as u64),
            })?,
            gas_limit: 1000000000,
            gas_fee_cap: Default::default(),
            gas_premium: Default::default(),
        },
        ApplyKind::Explicit,
        100,
    )?;

    assert_eq!(ExitCode::ErrIllegalArgument, ret.msg_receipt.exit_code);

    Ok(())
}

/// Setup can be called before a test to set up the scenario. This creates:
///
/// - A blockstore to contain all state data.
/// - A state tree, whose state root is returned.
/// - 10 accounts initialized with 1000 FIL each.
/// - A token actor deployed at the address specified in the return.
fn setup() -> Result<SetupRet> {
    let blockstore = MemoryBlockstore::default();

    let (mut state_tree, empty_hamt_cid) = init_state_tree(blockstore)?;

    // Put the WASM code into the blockstore.
    let code_cid = put_wasm_code(state_tree.store())?;

    let mut init_state = init_actor::State {
        address_map: empty_hamt_cid,
        next_id: 1000,
        network_name: String::from("mainnet"),
    };

    // Create 10 accounts.
    let accounts = put_secp256k1_accounts(&mut state_tree, &mut init_state, 10)?;

    // Save the init actor state.
    put_init_actor_state(&mut state_tree, &init_state)?;

    // Initialize the token state assigning 16000 tokens to each account.
    let token_actor_addr = put_initial_token_state(
        &mut state_tree,
        code_cid,
        accounts
            .iter()
            .map(|(id, _addr)| (*id, TokenAmount::from(16000 as u64)))
            .collect(),
    )?;

    let state_root = state_tree.flush().map_err(anyhow::Error::from)?;
    let blockstore = state_tree.consume();
    Ok(SetupRet {
        accounts,
        token_actor_addr,
        state_root,
        blockstore,
    })
}

/// Creates a DefaultMachine and a DefaultExecutor. It returns the latter.
fn create_executor(state_root: Cid, blockstore: MemoryBlockstore) -> impl Executor {
    let mut wasm_conf = wasmtime::Config::default();
    wasm_conf
        .cache_config_load_default()
        .expect("failed to load cache config");

    let machine = DefaultMachine::new(
        Config {
            max_call_depth: 4096,
            initial_pages: 0,
            max_pages: 1024,
            engine: wasm_conf,
            debug: true, // Enable debug mode by default.
        },
        0,
        TokenAmount::from(100 as u64),
        TokenAmount::from(100 as u64),
        NetworkVersion::V14,
        state_root,
        blockstore,
        CgoExterns::new(0),
    )
    .unwrap();

    DefaultExecutor::<DefaultKernel<DefaultCallManager<_>>>::new(machine)
}

/// Initializes a blank state tree.
fn init_state_tree(blockstore: MemoryBlockstore) -> Result<(StateTree<MemoryBlockstore>, Cid)> {
    let state_tree =
        StateTree::new(blockstore, StateTreeVersion::V4).map_err(anyhow::Error::from)?;

    // Insert an empty HAMT.
    let cid = Hamt::<_, Vec<u8>>::new(state_tree.store())
        .flush()
        .map_err(anyhow::Error::from)?;

    Ok((state_tree, cid))
}

/// Places the init actor's state into the state tree.
fn put_init_actor_state(
    state_tree: &mut StateTree<MemoryBlockstore>,
    init_state: &init_actor::State,
) -> Result<()> {
    let init_state_cid = state_tree.store().put_cbor(&init_state, Code::Blake2b256)?;
    let init_actor_state = ActorState {
        code: *INIT_ACTOR_CODE_ID,
        state: init_state_cid,
        sequence: 0,
        balance: Default::default(),
    };
    state_tree
        .set_actor(&INIT_ACTOR_ADDR, init_actor_state)
        .map_err(anyhow::Error::from)?;
    Ok(())
}

/// Inserts the WASM code for the actor into the blockstore.
fn put_wasm_code(blockstore: &MemoryBlockstore) -> Result<Cid> {
    let cid = blockstore.put(
        Code::Blake2b256,
        &Block {
            codec: IPLD_CODEC_RAW,
            data: wasm::WASM_BINARY.unwrap(),
        },
    )?;
    Ok(cid)
}

/// Inserts the initial token state into the blockstore into actor ID 2000.
fn put_initial_token_state(
    state_tree: &mut StateTree<impl Blockstore>,
    code_cid: Cid,
    initial_balances: Vec<(ActorID, TokenAmount)>,
) -> Result<Address> {
    let mut balances = Hamt::<_, BigIntDe, ActorID>::new(state_tree.store());
    for (id, amount) in initial_balances {
        balances.set(id, BigIntDe(amount))?;
    }

    let balances_cid = balances.flush()?;

    // State object.
    let state = State {
        name: NAME.to_owned(),
        symbol: SYMBOL.to_owned(),
        max_supply: TokenAmount::from(4200000 as u64),
        balances: balances_cid,
    };
    let state_cid = state_tree.store().put_cbor(&state, Code::Blake2b256)?;

    // State header.
    let addr = Address::new_id(TOKEN_ACTOR_ID);
    let actor_state = ActorState {
        code: code_cid,
        state: state_cid,
        sequence: 0,
        balance: Default::default(),
    };
    state_tree
        .set_actor(&addr, actor_state)
        .map_err(anyhow::Error::from)?;
    Ok(addr)
}

/// Inserts the specified number of accounts in the state tree, all with 1000 FIL,
/// returning their IDs and Addresses.
fn put_secp256k1_accounts(
    state_tree: &mut StateTree<impl Blockstore>,
    init_state: &mut init_actor::State,
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
        let state = fvm::account_actor::State {
            address: pub_key_addr.clone(),
        };

        let cid = state_tree.store().put_cbor(&state, Code::Blake2b256)?;

        let actor_state = ActorState {
            code: *ACCOUNT_ACTOR_CODE_ID,
            state: cid,
            sequence: 0,
            balance: TokenAmount::from(10u8) * TokenAmount::from(1000),
        };

        let id = init_state
            .map_address_to_new_id(state_tree.store(), &pub_key_addr)
            .map_err(anyhow::Error::from)?;
        state_tree
            .set_actor(&Address::new_id(id), actor_state)
            .map_err(anyhow::Error::from)?;
        ret.push((id, pub_key_addr));
    }
    Ok(ret)
}
