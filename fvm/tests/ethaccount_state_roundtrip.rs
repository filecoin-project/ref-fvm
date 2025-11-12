// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use cid::Cid;
use fvm_ipld_encoding::CborStore;

// Minimal view of EthAccount state for roundtrip (kept in sync with kernel implementation).
#[derive(
    fvm_ipld_encoding::tuple::Serialize_tuple,
    fvm_ipld_encoding::tuple::Deserialize_tuple,
    PartialEq,
    Debug,
)]
struct EthAccountStateView {
    delegate_to: Option<[u8; 20]>,
    auth_nonce: u64,
    evm_storage_root: Cid,
}

#[test]
fn ethaccount_state_roundtrip() {
    // Build an in-memory blockstore and a dummy CID as storage root.
    let bs = fvm_ipld_blockstore::MemoryBlockstore::new();
    // Use an identity multihash over a short payload to form a CID.
    use multihash_codetable::{Code, MultihashDigest};
    let mh = Code::Blake2b256.digest(b"root");
    let root = Cid::new_v1(fvm_ipld_encoding::DAG_CBOR, mh);

    let mut delegate = [0u8; 20];
    delegate.copy_from_slice(&[0xAB; 20]);

    let view = EthAccountStateView {
        delegate_to: Some(delegate),
        auth_nonce: 42,
        evm_storage_root: root,
    };

    // Encode to CBOR, then decode back.
    let cid = bs.put_cbor(&view, Code::Blake2b256).expect("put_cbor");
    let roundtrip: Option<EthAccountStateView> = bs.get_cbor(&cid).expect("get_cbor");
    assert!(roundtrip.is_some(), "expected state view to decode");
    assert_eq!(
        roundtrip.unwrap(),
        view,
        "decoded state must equal original"
    );
}
// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
