// Copyright 2021-2023 Protocol Labs
// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use anyhow::*;
use fvm_ipld_amt::{diff, Amt, Change, ChangeType};
use fvm_ipld_blockstore::MemoryBlockstore;
use itertools::Itertools;
use quickcheck::Arbitrary;
use quickcheck_macros::quickcheck;

#[derive(Debug, Clone)]
struct BitWidth2to18(u32);

impl Arbitrary for BitWidth2to18 {
    fn arbitrary(g: &mut quickcheck::Gen) -> Self {
        Self(*g.choose(&(2..=18).collect_vec()).unwrap())
    }
}

#[quickcheck]
fn test_simple_equals(BitWidth2to18(bit_width): BitWidth2to18) -> Result<()> {
    let prev_store = MemoryBlockstore::new();
    let curr_store = MemoryBlockstore::new();
    let mut a: Amt<String, _> = Amt::new_with_bit_width(prev_store, bit_width);
    let mut b: Amt<String, _> = Amt::new_with_bit_width(curr_store, bit_width);

    let changes = diff(&a, &b)?;
    ensure!(changes.is_empty());

    a.set(2, "foo".into())?;
    b.set(2, "foo".into())?;

    let changes = diff(&a, &b)?;
    ensure!(changes.is_empty());

    Ok(())
}

#[quickcheck]
fn test_simple_add(BitWidth2to18(bit_width): BitWidth2to18) -> Result<()> {
    let prev_store = MemoryBlockstore::new();
    let curr_store = MemoryBlockstore::new();
    let mut a: Amt<String, _> = Amt::new_with_bit_width(prev_store, bit_width);
    let mut b: Amt<String, _> = Amt::new_with_bit_width(curr_store, bit_width);
    a.set(2, "foo".into())?;
    a.flush()?;
    b.set(2, "foo".into())?;
    b.set(5, "bar".into())?;
    b.flush()?;

    let changes = diff(&a, &b)?;
    ensure!(changes.len() == 1);
    ensure!(
        changes
            == vec![Change {
                change_type: ChangeType::Add,
                key: 5,
                before: None,
                after: Some("bar".into())
            }]
    );

    Ok(())
}

#[quickcheck]
fn test_simple_remove(BitWidth2to18(bit_width): BitWidth2to18) -> Result<()> {
    let prev_store = MemoryBlockstore::new();
    let curr_store = MemoryBlockstore::new();
    let mut a: Amt<String, _> = Amt::new_with_bit_width(prev_store, bit_width);
    let mut b: Amt<String, _> = Amt::new_with_bit_width(curr_store, bit_width);
    a.set(2, "foo".into())?;
    a.set(5, "bar".into())?;
    a.flush()?;
    b.set(2, "foo".into())?;
    b.flush()?;

    let changes = diff(&a, &b)?;
    ensure!(changes.len() == 1);
    ensure!(
        changes
            == vec![Change {
                change_type: ChangeType::Remove,
                key: 5,
                before: Some("bar".into()),
                after: None,
            }]
    );

    Ok(())
}
