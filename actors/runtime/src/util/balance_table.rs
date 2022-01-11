// Copyright 2019-2022 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::bigint::bigint_ser::BigIntDe;
use fvm_shared::blockstore::Blockstore;
use fvm_shared::econ::TokenAmount;
use ipld_hamt::Error;
use num_traits::{Signed, Zero};

use crate::{make_empty_map, make_map_with_root_and_bitwidth, Map};

pub const BALANCE_TABLE_BITWIDTH: u32 = 6;

/// Balance table which handles getting and updating token balances specifically
pub struct BalanceTable<'a, BS>(Map<'a, BS, BigIntDe>);
impl<'a, BS> BalanceTable<'a, BS>
where
    BS: Blockstore,
{
    /// Initializes a new empty balance table
    pub fn new(bs: &'a BS) -> Self {
        Self(make_empty_map(bs, BALANCE_TABLE_BITWIDTH))
    }

    /// Initializes a balance table from a root Cid
    pub fn from_root(bs: &'a BS, cid: &Cid) -> Result<Self, Error> {
        Ok(Self(make_map_with_root_and_bitwidth(
            cid,
            bs,
            BALANCE_TABLE_BITWIDTH,
        )?))
    }

    /// Retrieve root from balance table
    pub fn root(&mut self) -> Result<Cid, Error> {
        self.0.flush()
    }

    /// Gets token amount for given address in balance table
    pub fn get(&self, key: &Address) -> Result<TokenAmount, Error> {
        if let Some(v) = self.0.get(&key.to_bytes())? {
            Ok(v.0.clone())
        } else {
            Ok(0.into())
        }
    }

    /// Adds token amount to previously initialized account.
    pub fn add(&mut self, key: &Address, value: &TokenAmount) -> Result<(), Error> {
        let prev = self.get(key)?;
        let sum = &prev + value;
        if sum.is_negative() {
            Err(format!("New balance in table cannot be negative: {}", sum).into())
        } else if sum.is_zero() && !prev.is_zero() {
            self.0.delete(&key.to_bytes())?;
            Ok(())
        } else {
            self.0.set(key.to_bytes().into(), BigIntDe(sum))?;
            Ok(())
        }
    }

    /// Subtracts up to the specified amount from a balance, without reducing the balance
    /// below some minimum.
    /// Returns the amount subtracted (always positive or zero).
    pub fn subtract_with_minimum(
        &mut self,
        key: &Address,
        req: &TokenAmount,
        floor: &TokenAmount,
    ) -> Result<TokenAmount, Error> {
        let prev = self.get(key)?;
        let available = std::cmp::max(TokenAmount::zero(), prev - floor);
        let sub: TokenAmount = std::cmp::min(&available, req).clone();

        if sub.is_positive() {
            self.add(key, &-sub.clone())?;
        }

        Ok(sub)
    }

    /// Subtracts value from a balance, and errors if full amount was not substracted.
    pub fn must_subtract(&mut self, key: &Address, req: &TokenAmount) -> Result<(), Error> {
        let prev = self.get(key)?;

        if req > &prev {
            Err("couldn't subtract the requested amount".into())
        } else {
            self.add(key, &-req)
        }
    }

    /// Returns total balance held by this balance table
    pub fn total(&self) -> Result<TokenAmount, Error> {
        let mut total = TokenAmount::default();

        self.0.for_each(|_, v: &BigIntDe| {
            total += &v.0;
            Ok(())
        })?;

        Ok(total)
    }
}
