// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::Result;
use fvm::executor::{ApplyKind, ApplyRet, Executor};
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::{strict_bytes, BytesSer, RawBytes};
use fvm_shared::address::Address;
use fvm_shared::econ::TokenAmount;
use fvm_shared::message::Message;
use fvm_shared::{ActorID, METHOD_CONSTRUCTOR};

use crate::tester::{BasicAccount, BasicTester};

pub const EAM_ADDRESS: Address = Address::new_id(10);
pub const DEFAULT_GAS: i64 = 10_000_000_000;

pub fn create_contract(
    tester: &mut BasicTester,
    owner: &mut BasicAccount,
    contract: &[u8],
    value: TokenAmount,
) -> Result<ApplyRet> {
    let create_msg = Message {
        from: owner.account.1,
        to: EAM_ADDRESS,
        gas_limit: DEFAULT_GAS,
        method_num: EAMMethod::CreateExternal as u64,
        params: RawBytes::serialize(BytesSer(contract)).unwrap(),
        sequence: owner.seqno,
        value,
        ..Message::default()
    };
    let create_mlen = create_msg.params.len();

    let create_res = tester
        .with_executor(|e| e.execute_message(create_msg, ApplyKind::Explicit, create_mlen))?;

    owner.seqno += 1;
    Ok(create_res)
}

pub fn invoke_contract(
    tester: &mut BasicTester,
    src: &mut BasicAccount,
    dest: Address,
    input_data: &[u8],
    gas: i64,
    value: TokenAmount,
) -> Result<ApplyRet> {
    let invoke_msg = Message {
        from: src.account.1,
        to: dest,
        sequence: src.seqno,
        gas_limit: gas,
        method_num: EVMMethod::InvokeContract as u64,
        params: RawBytes::serialize(BytesSer(input_data)).unwrap(),
        value,
        ..Message::default()
    };
    let invoke_mlen = invoke_msg.params.len();

    let invoke_res = tester
        .with_executor(|e| e.execute_message(invoke_msg, ApplyKind::Explicit, invoke_mlen))?;

    src.seqno += 1;
    Ok(invoke_res)
}

//////////////////////////////////////////////////////////////////////////////////////////
// we could theoretically have a dependency on the builtin actors themselves and reuse the
// actual definitions but it is currently a mess with the branches, so we just copy the types
/////////////////////////////////////////////////////////////////////////////////////////
#[repr(u64)]
pub enum EAMMethod {
    Constructor = METHOD_CONSTRUCTOR,
    Create = 2,
    Create2 = 3,
    CreateExternal = 4,
}

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

impl EthAddress {
    /// Returns an EVM-form ID address from actor ID.
    ///
    /// This is copied from the `evm` actor library.
    pub fn from_id(id: u64) -> EthAddress {
        let mut bytes = [0u8; 20];
        bytes[0] = 0xff;
        bytes[12..].copy_from_slice(&id.to_be_bytes());
        EthAddress(bytes)
    }
}

#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct CreateReturn {
    pub actor_id: ActorID,
    pub robust_address: Option<Address>,
    pub eth_address: EthAddress,
}
