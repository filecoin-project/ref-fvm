use std::collections::{HashMap, HashSet};
use std::fmt;

use anyhow::{anyhow, Result};
use cid::Cid;
use fmt::Display;
use fvm::executor::{ApplyKind, ApplyRet, DefaultExecutor, Executor};
use fvm::kernel::Context;
use fvm::machine::Machine;
use fvm::state_tree::StateTree;
use fvm_shared::address::Protocol;
use fvm_shared::blockstore::MemoryBlockstore;
use fvm_shared::crypto::signature::SECP_SIG_LEN;
use fvm_shared::encoding::Cbor;
use fvm_shared::message::Message;
use fvm_shared::receipt::Receipt;
use lazy_static::lazy_static;
use regex::Regex;
use walkdir::DirEntry;

use crate::vector::{MessageVector, Variant};
use crate::vm::{TestKernel, TestMachine};

lazy_static! {
    static ref SKIP_TESTS: Vec<Regex> = vec![
        // currently empty.
    ];
    /// The maximum parallelism when processing test vectors.
    pub static ref TEST_VECTOR_PARALLELISM: usize = std::env::var_os("TEST_VECTOR_PARALLELISM")
        .map(|s| {
            let s = s.to_str().unwrap();
            s.parse().expect("parallelism must be an integer")
        }).unwrap_or_else(num_cpus::get);
}

/// Checks if the file is a runnable vector.
pub fn is_runnable(entry: &DirEntry) -> bool {
    let file_name = match entry.path().to_str() {
        Some(file) => file,
        None => return false,
    };

    for rx in SKIP_TESTS.iter() {
        if rx.is_match(file_name) {
            println!("SKIPPING: {}", file_name);
            return false;
        }
    }

    file_name.ends_with(".json")
}

/// Compares the result of running a message with the expected result.
pub fn check_msg_result(expected_rec: &Receipt, ret: &ApplyRet, label: impl Display) -> Result<()> {
    let error = ret
        .backtrace
        .iter()
        .map(|e| {
            format!(
                "source: {:?}, code: {:?}, message: {:?}",
                e.source, e.code, e.message
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    let actual_rec = &ret.msg_receipt;
    let (expected, actual) = (expected_rec.exit_code, actual_rec.exit_code);
    if expected != actual {
        return Err(anyhow!(
            "exit code of msg {} did not match; expected: {:?}, got {:?}. Error: {}",
            label,
            expected,
            actual,
            error
        ));
    }

    let (expected, actual) = (&expected_rec.return_data, &actual_rec.return_data);
    if expected != actual {
        return Err(anyhow!(
            "return data of msg {} did not match; expected: {:?}, got {:?}",
            label,
            expected.as_slice(),
            actual.as_slice()
        ));
    }

    let (expected, actual) = (expected_rec.gas_used, actual_rec.gas_used);
    if expected != actual {
        return Err(anyhow!(
            "gas used of msg {} did not match; expected: {}, got {}",
            label,
            expected,
            actual
        ));
    }

    Ok(())
}

/// Compares the resulting state root with the expected state root. Currently,
/// this doesn't do much, but it could run a statediff.
pub fn compare_state_roots(bs: &MemoryBlockstore, root: &Cid, expected_root: &Cid) -> Result<()> {
    if root == expected_root {
        return Ok(());
    }

    let mut seen = HashSet::new();

    let mut actual = HashMap::new();
    StateTree::new_from_root(bs, root)
        .context("failed to load actual state tree")?
        .for_each(|addr, state| {
            actual.insert(addr, state.clone());
            Ok(())
        })?;

    let mut expected = HashMap::new();
    StateTree::new_from_root(bs, expected_root)
        .context("failed to load expected state tree")?
        .for_each(|addr, state| {
            expected.insert(addr, state.clone());
            Ok(())
        })?;
    for (k, va) in &actual {
        if seen.insert(k) {
            continue;
        }
        match expected.get(k) {
            Some(ve) if va != ve => {
                log::error!("actor {} has state {:?}, expected {:?}", k, va, ve)
            }
            None => log::error!("unexpected actor {}", k),
            _ => {}
        }
    }
    for (k, ve) in &expected {
        if seen.insert(k) {
            continue;
        }
        match actual.get(k) {
            Some(va) if va != ve => {
                log::error!("actor {} has state {:?}, expected {:?}", k, va, ve)
            }
            None => log::error!("missing actor {}", k),
            _ => {}
        }
    }

    return Err(anyhow!(
        "wrong post root cid; expected {}, but got {}",
        expected_root,
        root
    ));
}

/// Represents the result from running a vector.
pub enum VariantResult {
    /// The vector succeeded.
    Ok { id: String },
    /// A variant was skipped, due to the specified reason.
    Skipped { reason: String, id: String },
    /// A variant failed, due to the specified error.
    Failed { reason: anyhow::Error, id: String },
}

pub fn run_variant(
    bs: MemoryBlockstore,
    v: &MessageVector,
    variant: &Variant,
) -> anyhow::Result<VariantResult> {
    let id = variant.id.clone();

    // Construct the Machine.
    let machine = TestMachine::new_for_vector(v, variant, bs);
    let mut exec: DefaultExecutor<TestKernel> = DefaultExecutor::new(machine);

    // Apply all messages in the vector.
    for (i, m) in v.apply_messages.iter().enumerate() {
        let msg = Message::unmarshal_cbor(&m.bytes)?;

        // Execute the message.
        let mut raw_length = m.bytes.len();
        if msg.from.protocol() == Protocol::Secp256k1 {
            // 65 bytes signature + 1 byte type + 3 bytes for field info.
            raw_length += SECP_SIG_LEN + 4;
        }
        let ret = match exec.execute_message(msg, ApplyKind::Explicit, raw_length) {
            Ok(ret) => ret,
            Err(e) => return Ok(VariantResult::Failed { id, reason: e }),
        };

        // Compare the actual receipt with the expected receipt.
        let expected_receipt = &v.postconditions.receipts[i];
        if let Err(err) = check_msg_result(expected_receipt, &ret, i) {
            return Ok(VariantResult::Failed { id, reason: err });
        }
    }

    // Flush the machine, obtain the blockstore, and compare the
    // resulting state root with the expected state root.
    let final_root = match exec.flush() {
        Ok(cid) => cid,
        Err(err) => {
            return Ok(VariantResult::Failed {
                id,
                reason: err.context("flushing executor failed"),
            });
        }
    };

    let machine = match exec.consume() {
        Some(machine) => machine,
        None => {
            return Ok(VariantResult::Failed {
                id,
                reason: anyhow!("machine poisoned"),
            })
        }
    };

    let bs = machine.consume().consume();

    if let Err(err) = compare_state_roots(&bs, &final_root, &v.postconditions.state_tree.root_cid) {
        return Ok(VariantResult::Failed {
            id,
            reason: err.context("comparing state roots failed"),
        });
    }

    Ok(VariantResult::Ok { id })
}
