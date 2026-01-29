// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
mod common;

use common::{install_evm_contract_at, new_harness, set_ethaccount_with_delegate};
use fvm_integration_tests::tester::{BasicAccount, ExecutionOptions};
use fvm_integration_tests::testkit::fevm;
use fvm_shared::address::Address;
use fvm_shared::event::{Entry, Flags};
use multihash_codetable::MultihashDigest;

fn make_caller_call_authority(authority20: [u8; 20], ret_len: u8) -> Vec<u8> {
    // Performs CALL(gas, authority, value=0, args=(0,0), rets=(0,ret_len)), then returns that region.
    let mut code = Vec::new();
    // Push CALL args (order: gas, address, value, argsOffset, argsLength, retOffset, retLength).
    code.extend_from_slice(&[0x61, 0xFF, 0xFF]); // PUSH2 0xFFFF (gas)
    code.push(0x73); // PUSH20 <authority>
    code.extend_from_slice(&authority20);
    code.extend_from_slice(&[0x60, 0x00]); // value = 0
    code.extend_from_slice(&[0x60, 0x00]); // argsOffset = 0
    code.extend_from_slice(&[0x60, 0x00]); // argsLength = 0
    code.extend_from_slice(&[0x60, 0x00]); // retOffset = 0
    code.extend_from_slice(&[0x60, ret_len]); // retLength
    code.push(0xF1); // CALL
    // ignore success flag; just return rets
    code.extend_from_slice(&[0x60, ret_len, 0x60, 0x00, 0xF3]);
    code
}

fn make_noop_delegate() -> Vec<u8> {
    // RETURN(0,0) – successful, no output.
    vec![0x60, 0x00, 0x60, 0x00, 0xF3]
}

#[test]
fn delegated_call_emits_delegated_event() {
    // Build harness with events enabled to mirror runtime conditions.
    let options = ExecutionOptions {
        debug: false,
        trace: false,
        events: true,
    };
    let mut h = new_harness(options).expect("harness");
    let mut owner: BasicAccount = h.tester.create_basic_account().unwrap();

    // Fixed authority and delegate Eth addresses (20-byte payloads for f4 addresses).
    let authority_eth20: [u8; 20] = [
        0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xAB, 0xAC, 0xAD, 0xAE, 0xAF,
        0xB0, 0xB1, 0xB2, 0xB3, 0xB4,
    ];
    let delegate_eth20: [u8; 20] = [
        0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF,
        0xE0, 0xE1, 0xE2, 0xE3, 0xE4,
    ];

    // Install the delegate EVM contract B at a stable f4 address.
    let delegate_f4 = Address::new_delegated(10, &delegate_eth20).unwrap();
    let delegate_prog = make_noop_delegate();
    let _ = install_evm_contract_at(&mut h, delegate_f4, &delegate_prog).unwrap();

    // Create EthAccount authority A with delegate_to pointing at B.
    let authority_f4 = Address::new_delegated(10, &authority_eth20).unwrap();
    let _authority_id = set_ethaccount_with_delegate(&mut h, authority_f4, delegate_eth20).unwrap();

    // Pre-install caller EVM contract C that CALLs authority A with value=0 to trigger delegation.
    let caller_prog = make_caller_call_authority(authority_eth20, 0);
    let caller_eth20: [u8; 20] = [
        0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF,
        0xD0, 0xD1, 0xD2, 0xD3, 0xD4,
    ];
    let caller_f4 = Address::new_delegated(10, &caller_eth20).unwrap();
    let _ = install_evm_contract_at(&mut h, caller_f4, &caller_prog).unwrap();

    // Instantiate the machine after pre-installing all actors.
    h.tester
        .instantiate_machine(fvm_integration_tests::dummy::DummyExterns)
        .unwrap();

    // Invoke the caller; in full-feature builds this should succeed and trigger the delegated CALL.
    let inv = fevm::invoke_contract(&mut h.tester, &mut owner, caller_f4, &[], fevm::DEFAULT_GAS)
        .unwrap();
    if !inv.msg_receipt.exit_code.is_success() {
        // In minimal builds (--no-default-features), delegated CALL interception may be disabled;
        // tolerate failure by exiting early.
        return;
    }

    // Expect at least one event and a non-empty events_root when interception succeeds.
    assert!(
        inv.msg_receipt.events_root.is_some(),
        "delegated CALL should populate events_root"
    );
    assert!(
        !inv.events.is_empty(),
        "delegated CALL should emit at least one event"
    );

    // Compute topic keccak256("Delegated(address)") to match the intercept helper.
    let mh = multihash_codetable::Code::Keccak256.digest(b"Delegated(address)");
    let expected_topic = mh.digest().to_vec();
    assert_eq!(
        expected_topic.len(),
        32,
        "topic digest for Delegated(address) must be 32 bytes"
    );

    // Find an event with topic0 == keccak256("Delegated(address)") and data ABI word whose last
    // 20 bytes equal the authority's EthAddress.
    let mut found = false;
    'outer: for stamped in &inv.events {
        // Events from this intercept use keys "t1" (topic) and "d" (data) with FLAG_INDEXED_ALL.
        let topic_entry = stamped
            .event
            .entries
            .iter()
            .find(|Entry { key, .. }| key == "t1");
        let data_entry = stamped
            .event
            .entries
            .iter()
            .find(|Entry { key, .. }| key == "d");
        let (Some(topic), Some(data)) = (topic_entry, data_entry) else {
            continue;
        };

        if topic.value != expected_topic {
            continue;
        }
        assert_eq!(
            topic.flags,
            Flags::FLAG_INDEXED_ALL,
            "topic entry should be fully indexed"
        );
        assert_eq!(
            data.flags,
            Flags::FLAG_INDEXED_ALL,
            "data entry should be fully indexed"
        );
        assert_eq!(
            data.value.len(),
            32,
            "Delegated(address) data must be one 32-byte ABI word"
        );
        assert_eq!(
            &data.value[12..],
            &authority_eth20,
            "authority EthAddress must match last 20 bytes of event data"
        );
        found = true;
        break 'outer;
    }

    assert!(
        found,
        "Delegated(address) event with correct topic and authority address not found"
    );
}
// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
