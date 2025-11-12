// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::Cid;
use fvm::kernel::ActorOps as _;
use fvm::kernel::BlockRegistry;
use fvm::kernel::Kernel as _;
use fvm::kernel::default::DefaultKernel;
use fvm::machine::Machine as _;
use fvm::state_tree::ActorState;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::CborStore;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;

mod dummy;
use dummy::{DummyCallManager, DummyMachine};

#[derive(
    fvm_ipld_encoding::tuple::Serialize_tuple, fvm_ipld_encoding::tuple::Deserialize_tuple,
)]
struct EthAccountStateView {
    delegate_to: Option<[u8; 20]>,
    auth_nonce: u64,
    evm_storage_root: Cid,
}

fn new_kernel(cm: DummyCallManager) -> DefaultKernel<DummyCallManager> {
    <DefaultKernel<DummyCallManager> as fvm::kernel::Kernel>::new(
        cm,
        BlockRegistry::default(),
        10, // caller
        11, // actor_id
        0,  // method
        TokenAmount::from_atto(0u8),
        false, // read_only
    )
}

#[test]
fn get_eth_delegate_to_various() {
    // Build a dummy machine + call manager
    let (mut cm, _test_data) = DummyCallManager::new_stub();
    let store = cm.machine.state_tree.store();

    // Prepare EthAccount actor with delegate_to
    let authority_id = 1001u64;
    let eth_code = *cm.machine.builtin_actors().get_ethaccount_code();

    // Case 1: EOA with delegate_to set => Some
    let to_addr = [0xAA; 20];
    let st = EthAccountStateView {
        delegate_to: Some(to_addr),
        auth_nonce: 0,
        evm_storage_root: Cid::default(),
    };
    let st_cid = store
        .put_cbor(&st, multihash_codetable::Code::Blake2b256)
        .unwrap();
    let roundtrip: Option<EthAccountStateView> = store.get_cbor(&st_cid).unwrap();
    assert!(roundtrip.is_some());
    assert!(cm.machine.builtin_actors().is_ethaccount_actor(&eth_code));
    cm.machine.state_tree.set_actor(
        authority_id,
        ActorState::new(eth_code, st_cid, Default::default(), 0, None),
    );
    let actor = cm
        .machine
        .state_tree
        .get_actor(authority_id)
        .unwrap()
        .unwrap();
    assert_eq!(actor.code, eth_code);
    assert_eq!(actor.state, st_cid);

    let k = new_kernel(cm);
    let got = k.get_eth_delegate_to(authority_id).unwrap();
    assert_eq!(got, Some(to_addr));

    // Case 2: EOA with no delegate_to => None
    let (mut cm2, _) = DummyCallManager::new_stub();
    let store2 = cm2.machine.state_tree.store();
    let st2 = EthAccountStateView {
        delegate_to: None,
        auth_nonce: 0,
        evm_storage_root: Cid::default(),
    };
    let st2_cid = store2
        .put_cbor(&st2, multihash_codetable::Code::Blake2b256)
        .unwrap();
    cm2.machine.state_tree.set_actor(
        authority_id + 1,
        ActorState::new(eth_code, st2_cid, Default::default(), 0, None),
    );
    let k2 = new_kernel(cm2);
    let got2 = k2.get_eth_delegate_to(authority_id + 1).unwrap();
    assert_eq!(got2, None);

    // Case 3: non-EOA (e.g., placeholder) => None
    let (mut cm3, _) = DummyCallManager::new_stub();
    let placeholder = *cm3.machine.builtin_actors().get_placeholder_code();
    cm3.machine.state_tree.set_actor(
        authority_id + 2,
        ActorState::new(placeholder, Cid::default(), Default::default(), 0, None),
    );
    let k3 = new_kernel(cm3);
    let got3 = k3.get_eth_delegate_to(authority_id + 2).unwrap();
    assert_eq!(got3, None);
}
// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
