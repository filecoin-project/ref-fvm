use anyhow::Result;
use cid::Cid;
use fvm::machine::Manifest;
use fvm_integration_tests::bundle::import_bundle;
use fvm_integration_tests::tester::{BasicAccount, BasicTester, ExecutionOptions, Tester};
use fvm_ipld_blockstore::{Blockstore, MemoryBlockstore};
use fvm_ipld_encoding::CborStore;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use multihash_codetable::Code;

// Embedded actor bundle from builtin-actors (dev-dependency `actors`).
use actors; // fil_builtin_actors_bundle

// Minimal EthAccount state view mirroring kernel expectations.
#[derive(fvm_ipld_encoding::tuple::Serialize_tuple)]
pub struct EthAccountStateView {
    pub delegate_to: Option<[u8; 20]>,
    pub auth_nonce: u64,
    pub evm_storage_root: Cid,
}

pub struct Harness {
    pub tester: BasicTester,
    pub ethaccount_code: Cid,
    pub bundle_root: Cid,
}

pub fn new_harness(options: ExecutionOptions) -> Result<Harness> {
    // Build a blockstore and import the embedded bundle.
    let bs = MemoryBlockstore::default();
    let root = import_bundle(&bs, actors::BUNDLE_CAR)?;
    // Load manifest to fetch EthAccount code.
    let (ver, data_root): (u32, Cid) = bs
        .get_cbor(&root)?
        .expect("bundle manifest header not found");
    let manifest = Manifest::load(&bs, &data_root, ver)?;
    let ethaccount_code = *manifest.get_ethaccount_code();

    // Initialize a tester with this bundle.
    let mut tester = Tester::new(NetworkVersion::V21, StateTreeVersion::V5, root, bs)?;
    tester.options = Some(options);

    Ok(Harness { tester, ethaccount_code, bundle_root: root })
}

/// Create an EthAccount actor with the given authority delegated f4 address and EVM delegate (20 bytes).
/// Returns the assigned ActorID of the authority account.
pub fn set_ethaccount_with_delegate(
    h: &mut Harness,
    authority_addr: Address,
    delegate20: [u8; 20],
) -> Result<u64> {
    // Register the authority address to obtain an ActorID.
    let state_tree = h
        .tester
        .state_tree
        .as_mut()
        .expect("state tree should be present prior to instantiation");
    let authority_id = state_tree.register_new_address(&authority_addr).unwrap();

    // Persist minimal EthAccount state.
    let view = EthAccountStateView { delegate_to: Some(delegate20), auth_nonce: 0, evm_storage_root: Cid::default() };
    let st_cid = state_tree.store().put_cbor(&view, Code::Blake2b256)?;

    // Install the EthAccount actor state with delegated_address = authority_addr.
    let act = fvm::state_tree::ActorState::new(h.ethaccount_code, st_cid, TokenAmount::default(), 0, Some(authority_addr));
    state_tree.set_actor(authority_id, act);
    Ok(authority_id)
}


pub fn bundle_code_by_name(h: &Harness, name: &str) -> anyhow::Result<Option<cid::Cid>> {
    let store = h.tester.state_tree.as_ref().unwrap().store();
    let (ver, data_root): (u32, cid::Cid) = store.get_cbor(&h.bundle_root)?.expect("bundle header");
    if ver != 1 { return Ok(None); }
    let entries: Vec<(String, cid::Cid)> = store.get_cbor(&data_root)?.expect("manifest data");
    Ok(entries.into_iter().find(|(n, _)| n == name).map(|(_, c)| c))
}

pub fn install_evm_contract_at(
    h: &mut Harness,
    evm_addr: fvm_shared::address::Address,
    runtime: &[u8],
) -> anyhow::Result<u64> {
    use fvm_ipld_blockstore::Block;
    use multihash_codetable::Code as MhCode;

    // Resolve EVM actor code CID from the embedded bundle.
    let evm_code = bundle_code_by_name(h, "evm")?.expect("evm code in bundle");

    // Local types matching builtin-actors EVM state CBOR exactly.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
    struct BytecodeHash(#[serde(with = "fvm_ipld_encoding::strict_bytes")] [u8; 32]);

    #[derive(Clone, Copy, Debug, Eq, PartialEq, fvm_ipld_encoding::tuple::Serialize_tuple)]
    struct TransientDataLifespan {
        origin: fvm_shared::ActorID,
        nonce: u64,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, fvm_ipld_encoding::tuple::Serialize_tuple)]
    struct TransientData {
        transient_data_state: cid::Cid,
        transient_data_lifespan: TransientDataLifespan,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, fvm_ipld_encoding::tuple::Serialize_tuple)]
    struct Tombstone {
        origin: fvm_shared::ActorID,
        nonce: u64,
    }

    #[derive(fvm_ipld_encoding::tuple::Serialize_tuple)]
    struct EvmState {
        bytecode: cid::Cid,
        bytecode_hash: BytecodeHash,
        contract_state: cid::Cid,
        transient_data: Option<TransientData>,
        nonce: u64,
        tombstone: Option<Tombstone>,
        delegations: Option<cid::Cid>,
        delegation_nonces: Option<cid::Cid>,
        delegation_storage: Option<cid::Cid>,
    }

    // Access blockstore.
    let bs = h.tester.state_tree.as_ref().unwrap().store();

    // Persist runtime bytecode and compute keccak256 hash.
    let bytecode_blk = Block::new(fvm_ipld_encoding::IPLD_RAW, runtime);
    let bytecode_cid = bs.put(MhCode::Blake2b256, &bytecode_blk)?;
    let mut digest = [0u8; 32];
    {
        use multihash_codetable::MultihashDigest;
        let mh = multihash_codetable::Code::Keccak256.digest(runtime);
        digest.copy_from_slice(mh.digest());
    }

    // Create and persist an empty KAMT root for contract_state so the EVM can load it.
    let contract_state_cid = {
        use fvm_ipld_kamt::{id::Identity, Config as KamtConfig, Kamt};
        // Use the same config as the actor (bit_width=5, etc.). Key/value types are irrelevant for an empty map.
        let mut k: Kamt<_, [u8; 32], [u8; 32], Identity> =
            Kamt::new_with_config(bs.clone(), KamtConfig { min_data_depth: 0, bit_width: 5, max_array_width: 1 });
        k.flush()?
    };

    // Minimal EVM state; no transient data, no tombstone, no 7702 maps.
    let st = EvmState {
        bytecode: bytecode_cid,
        bytecode_hash: BytecodeHash(digest),
        contract_state: contract_state_cid,
        transient_data: None,
        nonce: 0,
        tombstone: None,
        delegations: None,
        delegation_nonces: None,
        delegation_storage: None,
    };

    // Persist state and install actor at requested address.
    let st_cid = bs.put_cbor(&st, multihash_codetable::Code::Blake2b256)?;
    let stree = h.tester.state_tree.as_mut().unwrap();
    let id = stree.register_new_address(&evm_addr).unwrap();
    let act = fvm::state_tree::ActorState::new(
        evm_code,
        st_cid,
        fvm_shared::econ::TokenAmount::default(),
        0,
        Some(evm_addr),
    );
    stree.set_actor(id, act);
    Ok(id)
}
