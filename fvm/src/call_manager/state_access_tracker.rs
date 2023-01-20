// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::cell::RefCell;
use std::iter;

use anyhow::Context;
use fvm_shared::address::{Address, Protocol};
use fvm_shared::ActorID;

use crate::gas::{GasTimer, GasTracker, PriceList};
use crate::history_map::HistoryMap;
use crate::kernel::{ClassifyResult, Result};

struct StateAccessLayer {
    addresses_height: usize,
    actors_height: usize,
}

pub struct StateAccessTracker {
    price_list: &'static PriceList,
    actors: RefCell<HistoryMap<ActorID, bool>>,
    addresses: RefCell<HistoryMap<Address, ()>>,
    layers: Vec<StateAccessLayer>,
}

impl StateAccessTracker {
    /// Create a new state access tracker.
    pub fn new(price_list: &'static PriceList) -> Self {
        Self {
            price_list,
            actors: RefCell::new(
                iter::zip(
                    price_list.preloaded_actors.iter().copied(),
                    iter::repeat(true),
                )
                .collect(),
            ),
            addresses: Default::default(),
            layers: Vec::new(),
        }
    }

    /// Begin a transaction.
    pub fn begin_transaction(&mut self) {
        self.layers.push(StateAccessLayer {
            addresses_height: self.addresses.borrow().history_len(),
            actors_height: self.actors.borrow().history_len(),
        })
    }

    /// End a transaction. If revert is true, the `StateAccessTracker` will forget all accesses from
    /// within the transaction and will re-charge for them in the future.
    pub fn end_transaction(&mut self, revert: bool) -> Result<()> {
        let layer = self
            .layers
            .pop()
            .context("state access tracker not in a transaction")
            .or_fatal()?;
        if revert {
            self.actors.get_mut().rollback(layer.actors_height);
            self.actors.get_mut().rollback(layer.addresses_height);
        }
        Ok(())
    }

    /// Charge for reading an actor's state, if not already charged.
    pub fn charge_actor_read(&self, gas_tracker: &GasTracker, actor: ActorID) -> Result<GasTimer> {
        match self.actors.borrow().get(&actor) {
            Some(_) => Ok(GasTimer::empty()),
            None => gas_tracker.apply_charge(self.price_list.on_actor_lookup()),
        }
    }

    /// Record that an actor's state was successfully read so that we don't charge for it again.
    pub fn record_actor_read(&self, actor: ActorID) {
        let mut actors = self.actors.borrow_mut();
        if actors.get(&actor).is_none() {
            actors.insert(actor, false)
        }
    }

    pub fn charge_actor_update(
        &self,
        gas_tracker: &GasTracker,
        actor: ActorID,
    ) -> Result<GasTimer> {
        match self.actors.borrow().get(&actor) {
            Some(true) => Ok(GasTimer::empty()),
            Some(false) => {
                let _ = gas_tracker.apply_charge(self.price_list.on_actor_lookup())?;
                gas_tracker.apply_charge(self.price_list.on_actor_update())
            }
            None => gas_tracker.apply_charge(self.price_list.on_actor_update()),
        }
    }

    pub fn record_actor_update(&self, actor: ActorID) {
        self.actors.borrow_mut().insert(actor, true)
    }

    pub fn charge_address_lookup(
        &self,
        gas_tracker: &GasTracker,
        addr: &Address,
    ) -> Result<GasTimer> {
        if addr.protocol() == Protocol::ID {
            return Ok(GasTimer::empty());
        }
        match self.addresses.borrow_mut().get(addr) {
            Some(_) => Ok(GasTimer::empty()),
            None => gas_tracker.apply_charge(self.price_list.on_resolve_address()),
        }
    }

    pub fn record_lookup_address(&self, addr: &Address) {
        if addr.protocol() == Protocol::ID {
            return;
        }
        self.addresses.borrow_mut().insert(*addr, ())
    }
}
