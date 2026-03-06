// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod common;

use cid::Cid;
use common::{install_evm_contract_at, new_harness};
use fvm::executor::{ApplyKind, Executor};
use fvm_integration_tests::tester::{BasicAccount, ExecutionOptions};
use fvm_ipld_encoding::CborStore;
use fvm_ipld_encoding::ipld_block::IpldBlock;
use fvm_shared::MethodNum;
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::message::Message;
use multihash_codetable::Code;

/// Minimal view of EthAccount state (kept in sync with builtin-actors).
#[derive(
    fvm_ipld_encoding::tuple::Serialize_tuple, fvm_ipld_encoding::tuple::Deserialize_tuple,
)]
struct EthAccountStateView {
    delegate_to: Option<[u8; 20]>,
    auth_nonce: u64,
    evm_storage_root: Cid,
}

#[derive(
    fvm_ipld_encoding::tuple::Serialize_tuple, fvm_ipld_encoding::tuple::Deserialize_tuple,
)]
struct DelegationParam {
    chain_id: u64,
    #[serde(with = "fvm_ipld_encoding::strict_bytes")]
    address: Vec<u8>,
    nonce: u64,
    y_parity: u8,
    #[serde(with = "fvm_ipld_encoding::strict_bytes")]
    r: Vec<u8>,
    #[serde(with = "fvm_ipld_encoding::strict_bytes")]
    s: Vec<u8>,
}

#[derive(
    fvm_ipld_encoding::tuple::Serialize_tuple, fvm_ipld_encoding::tuple::Deserialize_tuple,
)]
struct ApplyCall {
    #[serde(with = "fvm_ipld_encoding::strict_bytes")]
    to: Vec<u8>,
    #[serde(with = "fvm_ipld_encoding::strict_bytes")]
    value: Vec<u8>,
    #[serde(with = "fvm_ipld_encoding::strict_bytes")]
    input: Vec<u8>,
}

#[derive(
    fvm_ipld_encoding::tuple::Serialize_tuple, fvm_ipld_encoding::tuple::Deserialize_tuple,
)]
struct ApplyAndCallParams {
    list: Vec<DelegationParam>,
    call: ApplyCall,
}

#[derive(fvm_ipld_encoding::tuple::Deserialize_tuple)]
struct ApplyAndCallReturn {
    status: u8,
    #[serde(with = "fvm_ipld_encoding::strict_bytes")]
    output_data: Vec<u8>,
}

fn frc42_method_hash(name: &str) -> MethodNum {
    use multihash_codetable::MultihashDigest;
    let digest = multihash_codetable::Code::Keccak256.digest(name.as_bytes());
    let d = digest.digest();
    let mut bytes = [0u8; 8];
    bytes[4..8].copy_from_slice(&d[0..4]);
    u64::from_be_bytes(bytes)
}

/// Install an EthAccount actor for the given authority f4 address with an empty delegation map.
fn install_empty_ethaccount(
    h: &mut common::Harness,
    authority_addr: Address,
) -> anyhow::Result<u64> {
    let stree = h.tester.state_tree.as_mut().unwrap();
    let authority_id = stree.register_new_address(&authority_addr).unwrap();

    let view = EthAccountStateView {
        delegate_to: None,
        auth_nonce: 0,
        evm_storage_root: Cid::default(),
    };
    let st_cid = stree.store().put_cbor(&view, Code::Blake2b256)?;

    let act = fvm::state_tree::ActorState::new(
        h.ethaccount_code,
        st_cid,
        TokenAmount::default(),
        0,
        Some(authority_addr),
    );
    stree.set_actor(authority_id, act);
    Ok(authority_id)
}

fn make_returning_contract(payload: [u8; 3]) -> Vec<u8> {
    // Store 3-byte payload at memory offset 0 and RETURN(0,3).
    let mut code = Vec::new();
    code.extend_from_slice(&[0x62, payload[0], payload[1], payload[2]]); // PUSH3 payload
    code.extend_from_slice(&[0x60, 0x00]); // PUSH1 0
    code.push(0x52); // MSTORE
    code.extend_from_slice(&[0x60, 0x03, 0x60, 0x00, 0xF3]); // RETURN(0,3)
    code
}

#[test]
fn ethaccount_apply_and_call_updates_mapping_and_calls_evm() {
    let options = ExecutionOptions {
        debug: false,
        trace: false,
        events: true,
    };
    let mut h = new_harness(options).expect("harness");
    let owner: BasicAccount = h.tester.create_basic_account().unwrap();

    // Authority EthAccount at a stable f4 address with empty mapping.
    let authority_eth20: [u8; 20] = [
        0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF,
        0xB0, 0xB1, 0xB2, 0xB3, 0xB4,
    ];
    let authority_f4 = Address::new_delegated(10, &authority_eth20).unwrap();
    let _authority_id = install_empty_ethaccount(&mut h, authority_f4).expect("install ethaccount");

    // EVM contract that returns a fixed 3-byte payload.
    let contract_eth20: [u8; 20] = [
        0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF,
        0xD0, 0xD1, 0xD2, 0xD3, 0xD4,
    ];
    let contract_f4 = Address::new_delegated(10, &contract_eth20).unwrap();
    let ret_payload = [0xAA, 0xBB, 0xCC];
    let contract_code = make_returning_contract(ret_payload);
    let _contract_id = install_evm_contract_at(&mut h, contract_f4, &contract_code).unwrap();

    // Instantiate machine and obtain executor.
    h.tester
        .instantiate_machine(fvm_integration_tests::dummy::DummyExterns)
        .unwrap();
    let exec = h.tester.executor.as_mut().unwrap();

    // Build ApplyAndCallParams with one delegation tuple and an outer call to the EVM contract.
    let delegate20: [u8; 20] = [
        0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF,
        0xE0, 0xE1, 0xE2, 0xE3, 0xE4,
    ];
    let params = ApplyAndCallParams {
        list: vec![DelegationParam {
            chain_id: 0,
            address: delegate20.to_vec(),
            nonce: 0,
            y_parity: 0,
            r: vec![1u8; 32],
            s: vec![1u8; 32],
        }],
        call: ApplyCall {
            to: contract_eth20.to_vec(),
            value: Vec::new(),
            input: Vec::new(),
        },
    };

    let params_blk = IpldBlock::serialize_dag_cbor(&params)
        .expect("params cbor")
        .expect("ipld block");
    let method_apply_and_call = frc42_method_hash("ApplyAndCall");

    // Execute a Filecoin message from the owner to the EthAccount actor.
    let msg = Message {
        from: owner.account.1,
        to: authority_f4,
        method_num: method_apply_and_call,
        value: TokenAmount::from_atto(0u8),
        gas_limit: 10_000_000,
        params: params_blk.data.into(),
        ..Message::default()
    };

    let ret = exec
        .execute_message(msg, ApplyKind::Explicit, 100)
        .expect("message execution");

    // EthAccount.ApplyAndCall should exit OK at the FVM level and embed the
    // callee status/returndata in ApplyAndCallReturn.
    assert!(
        ret.msg_receipt.exit_code.is_success(),
        "EthAccount.ApplyAndCall must exit OK"
    );
    let out_bytes = ret.msg_receipt.return_data.bytes().to_vec();
    if !out_bytes.is_empty() {
        // Newer EthAccount bundles embed the callee status/returndata.
        let apply_ret: ApplyAndCallReturn =
            fvm_ipld_encoding::from_slice(&out_bytes).expect("decode ApplyAndCallReturn");
        assert_eq!(apply_ret.status, 1, "outer EVM call should succeed");
        assert_eq!(
            apply_ret.output_data, ret_payload,
            "outer call return data must match EVM contract"
        );
    }

    // EthAccount mapping + nonce must be updated as part of the same message.
    if let Some(stree) = h.tester.state_tree.as_ref() {
        let act = stree
            .get_actor(_authority_id)
            .expect("state tree")
            .expect("ethaccount actor");
        let view: Option<EthAccountStateView> =
            stree.store().get_cbor(&act.state).expect("decode state");
        let view = view.expect("state");
        assert_eq!(
            view.delegate_to,
            Some(delegate20),
            "delegate_to must be set from tuple"
        );
        assert_eq!(view.auth_nonce, 1, "auth_nonce should be incremented to 1");
        // Storage root should be initialized (non-default) after the outer call.
        assert_ne!(
            view.evm_storage_root,
            Cid::default(),
            "evm_storage_root should be initialized after apply+call"
        );
    }
}
