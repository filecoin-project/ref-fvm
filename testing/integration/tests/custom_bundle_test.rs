// Copyright 2026 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use cid::Cid;
use fvm::engine::EnginePool;
use fvm::executor::{ApplyKind, Executor};
use fvm::init_actor::INIT_ACTOR_ID;
use fvm::machine::{Machine, Manifest, NetworkConfig};
use fvm::state_tree::ActorState;
use fvm::system_actor::SYSTEM_ACTOR_ID;
use fvm_integration_tests::bundle;
use fvm_integration_tests::dummy::DummyExterns;
use fvm_integration_tests::tester::{BasicTester, Tester};
use fvm_integration_tests::testkit::fevm::{
    CreateReturn, DEFAULT_GAS, EAM_ADDRESS, EAMMethod, EVMMethod,
};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::{BytesSer, CborStore, RawBytes};
use fvm_shared::address::Address;
use fvm_shared::bigint::BigInt;
use fvm_shared::bigint::bigint_ser::BigIntSer;
use fvm_shared::econ::TokenAmount;
use fvm_shared::error::ExitCode;
use fvm_shared::message::Message;
use fvm_shared::sector::RegisteredPoStProof;
use fvm_shared::state::StateTreeVersion;
use fvm_shared::version::NetworkVersion;
use fvm_shared::{METHOD_CONSTRUCTOR, METHOD_SEND};

// An execution baseline, not a declaration of which network should install the bundle.
const TEST_NETWORK_VERSION: NetworkVersion = NetworkVersion::V29;
const REWARD_ADDRESS: Address = Address::new_id(2);
const POWER_ADDRESS: Address = Address::new_id(4);
const VERIFREG_ADDRESS: Address = Address::new_id(6);
const DATACAP_ADDRESS: Address = Address::new_id(7);

#[test]
#[ignore = "requires FVM_ACTORS_BUNDLE; see testing/integration/README.md"]
fn custom_bundle() -> Result<()> {
    let car_path = std::env::var_os("FVM_ACTORS_BUNDLE")
        .context("set FVM_ACTORS_BUNDLE to the CAR file to test")?;
    // Cargo runs tests from the package directory; relative bundle paths use the workspace root.
    let car_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(car_path);
    let car_path = car_path
        .canonicalize()
        .with_context(|| format!("failed to resolve bundle {}", car_path.display()))?;
    let check_filplus = env_flag("FVM_CHECK_FILPLUS")?;
    let expect_filplus_missing = env_flag("FVM_EXPECT_FILPLUS_MISSING")?;
    ensure!(
        !expect_filplus_missing || check_filplus,
        "FVM_EXPECT_FILPLUS_MISSING requires FVM_CHECK_FILPLUS=1"
    );
    println!(
        "Bundle: {}\nExecution baseline: NV{TEST_NETWORK_VERSION}",
        car_path.display()
    );

    let blockstore = MemoryBlockstore::default();
    let bytes = std::fs::read(&car_path)
        .with_context(|| format!("failed to read bundle {}", car_path.display()))?;
    let root = bundle::import_bundle(&blockstore, &bytes)
        .with_context(|| format!("failed to import bundle {}", car_path.display()))?;
    let (version, data): (u32, Cid) = blockstore
        .get_cbor(&root)?
        .context("bundle manifest root missing")?;
    let entries: Vec<(String, Cid)> = blockstore
        .get_cbor(&data)?
        .context("bundle manifest entries missing")?;
    let manifest = Manifest::load(&blockstore, &data, version)?;
    println!("CAR root: {root}\nManifest: {data} (version {version})");

    let mut names = HashSet::new();
    for (index, (name, code)) in entries.iter().enumerate() {
        let type_id = u32::try_from(index + 1)?;
        ensure!(names.insert(name), "duplicate actor name: {name}");
        ensure!(
            manifest.code_by_id(type_id) == Some(code) && manifest.id_by_code(code) == type_id,
            "inconsistent manifest mapping for {name} at type {type_id}"
        );
        println!("  type {type_id}: {name} {code}");
    }
    let code_for = |name: &str| -> Result<Cid> {
        entries
            .iter()
            .find_map(|(entry, code)| (entry == name).then_some(*code))
            .with_context(|| format!("smoke test requires the {name} actor"))
    };
    let evm_code = code_for("evm")?;
    let multisig_code = code_for("multisig")?;
    let reward_code = code_for("reward")?;
    let power_code = code_for("storagepower")?;
    let miner_code = code_for("storageminer")?;
    let verifreg_code = code_for("verifiedregistry").ok();
    let datacap_code = code_for("datacap").ok();
    if check_filplus {
        for (name, code) in [
            ("verifiedregistry", verifreg_code),
            ("datacap", datacap_code),
        ] {
            if expect_filplus_missing {
                ensure!(
                    code.is_none(),
                    "expected {name} to be absent, but the manifest includes it"
                );
            } else {
                ensure!(
                    code.is_some(),
                    "expected {name} in the manifest, but it is missing"
                );
            }
        }
        println!(
            "Fil+ expectation: {}",
            if expect_filplus_missing {
                "absent"
            } else {
                "present"
            }
        );
    }

    // The integration harness enables fvm/testing, which skips automatic builtin preloading.
    let config = NetworkConfig::new(TEST_NETWORK_VERSION);
    let pool = EnginePool::new((&config).into())?;
    let bytes = pool
        .acquire()
        .preload_all(&blockstore, manifest.builtin_actor_codes())?;
    println!(
        "Loaded all {} actor modules ({bytes} WASM bytes)",
        entries.len()
    );
    drop(pool);

    let mut tester: BasicTester =
        Tester::new(TEST_NETWORK_VERSION, StateTreeVersion::V5, root, blockstore)?;
    let (sender_id, sender_robust) = tester.create_account()?;
    let sender = Address::new_id(sender_id);
    let tree = tester.state_tree.as_mut().context("state tree missing")?;
    let mut sender_state = tree.get_actor(sender_id)?.context("sender missing")?;
    sender_state.balance = TokenAmount::from_whole(1000u64);
    tree.set_actor(sender_id, sender_state);
    for (address, code) in [(REWARD_ADDRESS, reward_code), (POWER_ADDRESS, power_code)] {
        tree.set_actor(address.id()?, ActorState::new_empty(code, None));
    }
    if check_filplus {
        for (address, code) in [
            (VERIFREG_ADDRESS, verifreg_code),
            (DATACAP_ADDRESS, datacap_code),
        ] {
            if let Some(code) = code {
                tree.set_actor(address.id()?, ActorState::new_empty(code, None));
            }
        }
    }
    tester.instantiate_machine_with_config(
        DummyExterns,
        |_| {},
        |mc| {
            // Zero fees keep sender funding independent of gas charges.
            mc.set_base_fee(TokenAmount::from_atto(0));
        },
    )?;

    let mut constructors = vec![
        (POWER_ADDRESS, RawBytes::default()),
        (
            REWARD_ADDRESS,
            RawBytes::serialize(BigIntSer(&BigInt::from(0)))?,
        ),
    ];
    if check_filplus {
        if verifreg_code.is_some() {
            constructors.push((VERIFREG_ADDRESS, RawBytes::serialize(sender)?));
        }
        if datacap_code.is_some() {
            constructors.push((DATACAP_ADDRESS, RawBytes::serialize(VERIFREG_ADDRESS)?));
        }
    }
    for (to, params) in constructors {
        execute(
            &mut tester,
            Message {
                from: Address::new_id(SYSTEM_ACTOR_ID),
                to,
                method_num: METHOD_CONSTRUCTOR,
                params,
                ..Message::default()
            },
            ApplyKind::Implicit,
            ExitCode::OK,
        )?;
    }
    println!("Singleton WASM constructors passed");

    let pubkey = call(
        &mut tester,
        sender,
        sender,
        2, // Account.PubkeyAddress
        RawBytes::default(),
        TokenAmount::from_atto(0),
    )?;
    ensure!(
        pubkey.deserialize::<Address>()? == sender_robust,
        "wrong account pubkey"
    );

    let bls = Address::new_bls(&[0x44; 48])?;
    call(
        &mut tester,
        sender,
        bls,
        METHOD_SEND,
        RawBytes::default(),
        TokenAmount::from_atto(5),
    )?;
    assert_actor_code(&tester, bls, manifest.get_account_code())?;
    println!("Account method and automatic account creation passed");

    let delegated = Address::new_delegated(EAM_ADDRESS.id()?, &[0x11; 20])?;
    call(
        &mut tester,
        sender,
        delegated,
        METHOD_SEND,
        RawBytes::default(),
        TokenAmount::from_whole(1u64),
    )?;
    assert_actor_code(&tester, delegated, manifest.get_placeholder_code())?;
    call(
        &mut tester,
        delegated,
        sender,
        METHOD_SEND,
        RawBytes::default(),
        TokenAmount::from_atto(1),
    )?;
    assert_actor_code(&tester, delegated, manifest.get_ethaccount_code())?;
    println!("Placeholder creation and EthAccount promotion passed");

    // The constructor returns runtime bytecode that returns the uint256 value 42.
    let initcode = hex::decode("600a600c600039600a6000f3602a60005260206000f3")?;
    let created = call(
        &mut tester,
        delegated,
        EAM_ADDRESS,
        EAMMethod::CreateExternal as u64,
        RawBytes::serialize(BytesSer(&initcode))?,
        TokenAmount::from_atto(0),
    )?;
    let created: CreateReturn = created.deserialize()?;
    let contract = Address::new_id(created.actor_id);
    assert_actor_code(&tester, contract, &evm_code)?;
    let returned = call(
        &mut tester,
        delegated,
        contract,
        EVMMethod::InvokeContract as u64,
        RawBytes::serialize(BytesSer(&[]))?,
        TokenAmount::from_atto(0),
    )?;
    let returned: RawBytes = returned.deserialize()?;
    let mut expected = [0; 32];
    expected[31] = 42;
    ensure!(
        returned.bytes() == expected,
        "wrong EVM return value: {returned:?}"
    );
    println!("EAM deployment and EVM execution returning 42 passed");

    let constructor = RawBytes::serialize((vec![sender], 1u64, 0i64, 0i64))?;
    let created = call(
        &mut tester,
        sender,
        Address::new_id(INIT_ACTOR_ID),
        2, // Init.Exec
        RawBytes::serialize((multisig_code, constructor))?,
        TokenAmount::from_atto(0),
    )?;
    let (multisig, _robust): (Address, Address) = created.deserialize()?;
    assert_actor_code(&tester, multisig, &multisig_code)?;
    println!("Init.Exec multisig creation passed");

    // Mainnet and calibration bundles support the 32 GiB Window PoSt proof type.
    let params = RawBytes::serialize((
        sender,
        bls,
        RegisteredPoStProof::StackedDRGWindow32GiBV1P1,
        BytesSer(&[]),
        Vec::<RawBytes>::new(),
    ))?;
    let created = call(
        &mut tester,
        sender,
        POWER_ADDRESS,
        2, // Power.CreateMiner
        params,
        TokenAmount::from_whole(100u64),
    )?;
    let (miner, _robust): (Address, Address) = created.deserialize()?;
    assert_actor_code(&tester, miner, &miner_code)?;
    let controls = call(
        &mut tester,
        sender,
        miner,
        2, // Miner.ControlAddresses
        RawBytes::default(),
        TokenAmount::from_atto(0),
    )?;
    let (owner, worker, controls): (Address, Address, Vec<Address>) = controls.deserialize()?;
    let worker_id = tester
        .executor
        .as_ref()
        .context("executor missing")?
        .state_tree()
        .lookup_id(&bls)?
        .context("worker address missing")?;
    ensure!(
        owner == sender && worker == Address::new_id(worker_id) && controls.is_empty(),
        "miner has unexpected control addresses"
    );
    println!("Power.CreateMiner and Miner.ControlAddresses passed");

    if check_filplus {
        check_filplus_actors(
            &mut tester,
            sender,
            miner,
            verifreg_code,
            datacap_code,
            expect_filplus_missing,
        )?;
    }

    let final_root = tester
        .executor
        .as_mut()
        .context("executor missing")?
        .flush()?;
    println!("Bundle loading and WASM execution passed; final state root: {final_root}");
    Ok(())
}

fn env_flag(name: &str) -> Result<bool> {
    match std::env::var_os(name) {
        None => Ok(false),
        Some(value) if value == "1" || value == "true" => Ok(true),
        Some(value) if value == "0" || value == "false" => Ok(false),
        Some(_) => bail!("{name} must be 1/true or 0/false"),
    }
}

fn call(
    tester: &mut BasicTester,
    from: Address,
    to: Address,
    method_num: u64,
    params: RawBytes,
    value: TokenAmount,
) -> Result<RawBytes> {
    execute(
        tester,
        Message {
            from,
            to,
            method_num,
            params,
            value,
            ..Message::default()
        },
        ApplyKind::Explicit,
        ExitCode::OK,
    )
}

fn execute(
    tester: &mut BasicTester,
    mut message: Message,
    kind: ApplyKind,
    expected: ExitCode,
) -> Result<RawBytes> {
    let executor = tester.executor.as_mut().context("executor missing")?;
    if matches!(kind, ApplyKind::Explicit) {
        message.sequence = executor
            .state_tree()
            .get_actor_by_address(&message.from)?
            .context("sender missing")?
            .sequence;
    }
    message.gas_limit = DEFAULT_GAS;
    let (method_num, to) = (message.method_num, message.to);
    let raw_length = fvm_ipld_encoding::to_vec(&message)?.len();
    let result = executor.execute_message(message, kind, raw_length)?;
    ensure!(
        result.msg_receipt.exit_code == expected,
        "method {method_num} to {to}: expected {expected}, got {}; failure={:?}",
        result.msg_receipt.exit_code,
        result.failure_info
    );
    println!(
        "  method {method_num} to {to}: exit={expected}, gas={}",
        result.msg_receipt.gas_used
    );
    Ok(result.msg_receipt.return_data)
}

fn check_filplus_actors(
    tester: &mut BasicTester,
    sender: Address,
    miner: Address,
    verifreg_code: Option<Cid>,
    datacap_code: Option<Cid>,
    expect_missing: bool,
) -> Result<()> {
    // GetClaims reports the missing claim in its batch result, with a successful top-level exit.
    let no_claims = RawBytes::serialize((
        (0u32, vec![(0u32, ExitCode::USR_NOT_FOUND)]),
        Vec::<()>::new(),
    ))?;
    for (to, method_num, params, expected_return, present_exit) in [
        (
            VERIFREG_ADDRESS,
            2199871187, // Verifreg.GetClaims
            RawBytes::serialize((miner.id()?, vec![1u64]))?,
            no_claims,
            ExitCode::OK,
        ),
        (
            DATACAP_ADDRESS,
            48890204, // DataCap.Name
            RawBytes::default(),
            RawBytes::serialize("DataCap")?,
            ExitCode::OK,
        ),
        (
            DATACAP_ADDRESS,
            3261979605, // DataCap.Balance
            RawBytes::serialize(sender)?,
            RawBytes::serialize(TokenAmount::from_atto(0))?,
            ExitCode::OK,
        ),
        // The sender is not the governor; live and frozen implementations both forbid this call.
        (
            DATACAP_ADDRESS,
            116935346, // DataCap.Mint
            RawBytes::serialize((sender, TokenAmount::from_atto(0), Vec::<Address>::new()))?,
            RawBytes::default(),
            ExitCode::USR_FORBIDDEN,
        ),
    ] {
        let expected = if expect_missing {
            ExitCode::SYS_INVALID_RECEIVER
        } else {
            present_exit
        };
        let returned = execute(
            tester,
            Message {
                from: sender,
                to,
                method_num,
                params,
                ..Message::default()
            },
            ApplyKind::Explicit,
            expected,
        )?;
        if expected.is_success() {
            ensure!(
                returned == expected_return,
                "unexpected return from method {method_num} at {to}"
            );
        }
    }
    for (address, code) in [
        (VERIFREG_ADDRESS, verifreg_code),
        (DATACAP_ADDRESS, datacap_code),
    ] {
        if expect_missing {
            ensure!(
                tester
                    .executor
                    .as_ref()
                    .context("executor missing")?
                    .state_tree()
                    .get_actor(address.id()?)?
                    .is_none(),
                "absent actor {address} was created"
            );
        } else {
            assert_actor_code(
                tester,
                address,
                &code.context("required Fil+ code CID missing")?,
            )?;
        }
    }
    println!(
        "Fil+ checks passed: expected {}",
        if expect_missing {
            "absent actors and missing-receiver errors"
        } else {
            "present actors and verified method results"
        }
    );
    Ok(())
}

fn assert_actor_code(tester: &BasicTester, address: Address, expected: &Cid) -> Result<()> {
    let actor = tester
        .executor
        .as_ref()
        .context("executor missing")?
        .state_tree()
        .get_actor_by_address(&address)?
        .context("actor missing")?;
    ensure!(
        actor.code == *expected,
        "actor {address} has wrong code CID"
    );
    Ok(())
}
