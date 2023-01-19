// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

#![allow(dead_code)]

use fvm::executor::{ApplyKind, ApplyRet, Executor};
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::{strict_bytes, BytesSer, RawBytes};
use fvm_shared::address::Address;
use fvm_shared::message::Message;
use fvm_shared::{ActorID, METHOD_CONSTRUCTOR};

use crate::{Account, BasicTester};

pub fn create_contract(tester: &mut BasicTester, owner: &mut Account, contract: &[u8]) -> ApplyRet {
    let create_msg = Message {
        from: owner.account.1,
        to: Address::new_id(10), // EAM
        gas_limit: 10_000_000_000,
        method_num: EAMMethod::CreateExternal as u64,
        params: RawBytes::serialize(BytesSer(contract)).unwrap(),
        sequence: owner.seqno,
        ..Message::default()
    };
    let create_mlen = create_msg.params.len();

    let create_res = tester
        .executor
        .as_mut()
        .unwrap()
        .execute_message(create_msg, ApplyKind::Explicit, create_mlen)
        .unwrap();

    owner.seqno += 1;
    create_res
}

pub fn invoke_contract(
    tester: &mut BasicTester,
    src: &mut Account,
    dest: Address,
    input_data: &[u8],
    gas: i64,
) -> ApplyRet {
    let invoke_msg = Message {
        from: src.account.1,
        to: dest,
        sequence: src.seqno,
        gas_limit: gas,
        method_num: EVMMethod::InvokeContract as u64,
        params: RawBytes::serialize(BytesSer(input_data)).unwrap(),
        ..Message::default()
    };
    let invoke_mlen = invoke_msg.params.len();

    let invoke_res = tester
        .executor
        .as_mut()
        .unwrap()
        .execute_message(invoke_msg, ApplyKind::Explicit, invoke_mlen)
        .unwrap();

    src.seqno += 1;
    invoke_res
}

//////////////////////////////////////////////////////////////////////////////////////////
// we could theoretically have a dependency on the builtin actors themselves and reuse the
// actual definitions but it is currently a mess with the branches, so we just copy the types
/////////////////////////////////////////////////////////////////////////////////////////
#[allow(dead_code)]
#[repr(u64)]
pub enum EAMMethod {
    Constructor = METHOD_CONSTRUCTOR,
    Create = 2,
    Create2 = 3,
    CreateExternal = 4,
}

#[allow(dead_code)]
#[repr(u64)]
pub enum EVMMethod {
    Constructor = METHOD_CONSTRUCTOR,
    Resurrect = 2,
    GetBytecode = 3,
    GetBytecodeHash = 4,
    GetStorageAt = 5,
    InvokeContractDelegate = 6,
    // it is very unfortunate but the hasher creates a circular dependency, so we use the raw
    // number.
    //InvokeContract = frc42_dispatch::method_hash!("InvokeEVM"),
    InvokeContract = 3844450837,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EthAddress(#[serde(with = "strict_bytes")] pub [u8; 20]);

#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct CreateReturn {
    pub actor_id: ActorID,
    pub robust_address: Option<Address>,
    pub eth_address: EthAddress,
}
