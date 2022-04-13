use std::fmt;

use anyhow::{anyhow, Result};
use cid::Cid;
use fmt::Display;
use fvm::executor::{ApplyKind, ApplyRet, DefaultExecutor, Executor};
use fvm::kernel::Context;
use fvm::machine::{Engine, Machine};
use fvm::state_tree::{ActorState, StateTree};
use fvm_ipld_blockstore::MemoryBlockstore;
use fvm_ipld_encoding::{Cbor, CborStore};
use fvm_shared::address::Protocol;
use fvm_shared::crypto::signature::SECP_SIG_LEN;
use fvm_shared::message::Message;
use fvm_shared::receipt::Receipt;
use lazy_static::lazy_static;
use libipld_core::ipld::Ipld;
use regex::Regex;
use walkdir::DirEntry;

use crate::vector::{MessageVector, Variant};
use crate::vm::{TestKernel, TestMachine};

lazy_static! {
    static ref SKIP_TESTS: Vec<Regex> = vec![
        // currently empty.
    ];
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
fn check_msg_result(expected_rec: &Receipt, ret: &ApplyRet, label: impl Display) -> Result<()> {
    let error = ret
        .failure_info
        .as_ref()
        .map(|e| e.to_string())
        .unwrap_or_else(|| "no error".into());
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

fn compare_actors(
    bs: &MemoryBlockstore,
    identifier: impl Display,
    actual: Option<ActorState>,
    expected: Option<ActorState>,
) -> Result<()> {
    if actual == expected {
        return Ok(());
    }
    log::error!(
        "{} actor state differs: {:?} != {:?}",
        identifier,
        actual,
        expected
    );

    match (actual, expected) {
        (Some(a), Some(e)) if a.state != e.state => {
            let a_root: Vec<Ipld> = bs.get_cbor(&a.state)?.unwrap();
            let e_root: Vec<Ipld> = bs.get_cbor(&e.state)?.unwrap();
            if a_root.len() != e_root.len() {
                log::error!("states have different numbers of fields")
            } else {
                for (f, (af, ef)) in a_root.iter().zip(e_root.iter()).enumerate() {
                    if af != ef {
                        log::error!("mismatched field {}: {:#?} != {:#?}", f, af, ef);
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Compares the state-root with the postcondition state-root in the test vector. If they don't
/// match, it performs a basic actor & state-diff of the message senders and receivers in the test
/// vector, along with all system actors.
fn compare_state_roots(bs: &MemoryBlockstore, root: &Cid, vector: &MessageVector) -> Result<()> {
    if root == &vector.postconditions.state_tree.root_cid {
        return Ok(());
    }

    let actual_st =
        StateTree::new_from_root(bs, root).context("failed to load actual state tree")?;
    let expected_st = StateTree::new_from_root(bs, &vector.postconditions.state_tree.root_cid)
        .context("failed to load expected state tree")?;

    // We only compare system actors and the send/receiver actor as we don't know what other actors
    // might exist in the state-tree (it's usually incomplete).

    for m in &vector.apply_messages {
        let msg = Message::unmarshal_cbor(&m.bytes)?;
        let actual_actor = actual_st.get_actor(&msg.from)?;
        let expected_actor = expected_st.get_actor(&msg.from)?;
        compare_actors(bs, "sender", actual_actor, expected_actor)?;

        let actual_actor = actual_st.get_actor(&msg.to)?;
        let expected_actor = expected_st.get_actor(&msg.to)?;
        compare_actors(bs, "receiver", actual_actor, expected_actor)?;
    }

    // All system actors
    for id in 0..100 {
        let expected_actor = match expected_st.get_actor_id(id) {
            Ok(act) => act,
            Err(_) => continue, // we don't expect it anyways.
        };
        let actual_actor = actual_st.get_actor_id(id)?;
        compare_actors(
            bs,
            format_args!("builtin {}", id),
            actual_actor,
            expected_actor,
        )?;
    }

    return Err(anyhow!(
        "wrong post root cid; expected {}, but got {}",
        &vector.postconditions.state_tree.root_cid,
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
    engine: &Engine,
    check_correctness: bool,
) -> anyhow::Result<VariantResult> {
    let id = variant.id.clone();

    // Construct the Machine.
    let machine = TestMachine::new_for_vector(v, variant, bs, engine);
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

        if check_correctness {
            // Compare the actual receipt with the expected receipt.
            let expected_receipt = &v.postconditions.receipts[i];
            if let Err(err) = check_msg_result(expected_receipt, &ret, i) {
                return Ok(VariantResult::Failed { id, reason: err });
            }
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
    if check_correctness {
        let bs = machine.consume().consume();

        if let Err(err) = compare_state_roots(&bs, &final_root, v) {
            return Ok(VariantResult::Failed {
                id,
                reason: err.context("comparing state roots failed"),
            });
        }
    }

    Ok(VariantResult::Ok { id })
}
