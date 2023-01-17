use anyhow::anyhow;
use fvm::executor::{ApplyKind, Executor};
use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::tester::{Account, Tester};
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::{strict_bytes, tuple::*, BytesDe, BytesSer, RawBytes};
use fvm_shared::{address::Address, message::Message, ActorID, METHOD_CONSTRUCTOR};

use crate::Options;

pub fn run<B: Blockstore>(
    tester: &mut Tester<B, DummyExterns>,
    options: &Options,
    contract: &[u8],
    entrypoint: &[u8],
    params: &[u8],
    gas: i64,
) -> anyhow::Result<()> {
    let accounts: [Account; 1] = tester.create_accounts().unwrap();
    tester
        .instantiate_machine_with_config(
            DummyExterns,
            |cfg| cfg.actor_debugging = options.debug,
            |mc| mc.tracing = options.trace,
        )
        .unwrap();

    let create_params_ser = Vec::from(contract);
    let create_mlen = create_params_ser.len();
    let create_msg = Message {
        from: accounts[0].1,
        to: Address::new_id(10),
        gas_limit: 10_000_000_000,
        method_num: EAMMethod::CreateExternal as u64,
        params: RawBytes::serialize(BytesSer(&create_params_ser)).unwrap(),
        ..Message::default()
    };

    let create_res = tester
        .executor
        .as_mut()
        .unwrap()
        .execute_message(create_msg, ApplyKind::Explicit, create_mlen)
        .unwrap();

    if create_res.msg_receipt.exit_code.value() != 0 {
        return Err(anyhow!(
            "actor creation failed: {}",
            create_res.msg_receipt.exit_code
        ));
    }

    let create_return: CreateReturn = create_res.msg_receipt.return_data.deserialize().unwrap();

    // invoke contract
    let mut input_data = Vec::from(entrypoint);
    let mut input_params = Vec::from(params);
    input_data.append(&mut input_params);
    let invoke_msg = Message {
        from: accounts[0].1,
        to: Address::new_id(create_return.actor_id),
        sequence: 1,
        gas_limit: gas,
        method_num: EVMMethod::InvokeContract as u64,
        params: RawBytes::serialize(BytesSer(&input_data)).unwrap(),
        ..Message::default()
    };
    let invoke_mlen = invoke_msg.params.len();

    let invoke_res = tester
        .executor
        .as_mut()
        .unwrap()
        .execute_message(invoke_msg, ApplyKind::Explicit, invoke_mlen)
        .unwrap();

    if !invoke_res.msg_receipt.exit_code.is_success() {
        return Err(anyhow!(
            "contract invocation failed: {} -- {:?}",
            invoke_res.msg_receipt.exit_code,
            invoke_res.failure_info,
        ));
    }

    let BytesDe(invoke_result) = invoke_res.msg_receipt.return_data.deserialize().unwrap();

    println!("Result: {}", hex::encode(invoke_result));
    println!("Gas Used: {}", invoke_res.msg_receipt.gas_used);

    if options.trace {
        println!("Execution trace:");
        for tr in invoke_res.exec_trace {
            println!("{:?}", tr)
        }
    }

    if options.events {
        println!("Execution events:");
        for evt in invoke_res.events {
            println!("{:?}", evt)
        }
    }

    Ok(())
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

#[repr(u64)]
pub enum EVMMethod {
    Constructor = METHOD_CONSTRUCTOR,
    Resurrect = 2,
    GetBytecode = 3,
    GetBytecodeHash = 4,
    GetStorageAt = 5,
    InvokeContractDelegate = 6,
    InvokeContract = frc42_dispatch::method_hash!("InvokeEVM"),
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EthAddress(#[serde(with = "strict_bytes")] pub [u8; 20]);

#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct Create2Params {
    #[serde(with = "strict_bytes")]
    pub initcode: Vec<u8>,
    #[serde(with = "strict_bytes")]
    pub salt: [u8; 32],
}

#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct CreateReturn {
    pub actor_id: ActorID,
    pub robust_address: Address,
    pub eth_address: EthAddress,
}
