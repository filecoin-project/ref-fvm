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
            // Already written.
            Some(true) => Ok(GasTimer::empty()),
            // Already read, but not written.
            Some(false) => gas_tracker.apply_charge(self.price_list.on_actor_update()),
            // Never touched, charge both.
            None => {
                let _ = gas_tracker.apply_charge(self.price_list.on_actor_lookup())?;
                gas_tracker.apply_charge(self.price_list.on_actor_update())
            }
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

#[cfg(test)]
mod test {
    use fvm_shared::address::{Address, SECP_PUB_LEN};
    use fvm_shared::version::NetworkVersion;
    use num_traits::Zero;

    use super::StateAccessTracker;
    use crate::gas::{price_list_by_network_version, Gas, GasCharge, GasTracker, PriceList};

    const GAS_LIMIT: Gas = Gas::new(10_000_000_000);

    fn new_tracker() -> (&'static PriceList, StateAccessTracker, GasTracker) {
        let pl = price_list_by_network_version(NetworkVersion::V18);
        (
            pl,
            StateAccessTracker::new(pl),
            GasTracker::new(GAS_LIMIT, Gas::zero(), true),
        )
    }

    fn assert_charges(gas: &GasTracker, charges: impl IntoIterator<Item = GasCharge>) {
        let mut iter_trace = gas.drain_trace().fuse();
        let mut iter_exp = charges.into_iter().fuse();
        if let Some((idx, (trace, exp))) = std::iter::zip(iter_trace.by_ref(), iter_exp.by_ref())
            .enumerate()
            .find(|(_, (trace, exp))| trace != exp)
        {
            panic!(
                "expected {idx} gas charge {} ({}), got {} ({})",
                exp.name,
                exp.total(),
                trace.name,
                trace.total(),
            )
        }
        if let Some(charge) = iter_trace.next() {
            panic!(
                "unexpected remaining gas charge {} ({})",
                charge.name,
                charge.total(),
            )
        }
        if let Some(charge) = iter_exp.next() {
            panic!("expected gas charge {} ({})", charge.name, charge.total())
        }
    }

    #[test]
    fn test_state_access_tracker_actor() {
        let (pl, mut state, gas) = new_tracker();
        state.begin_transaction();

        // Read charges for lookup.
        let _ = state.charge_actor_read(&gas, 101).unwrap();
        assert_charges(&gas, [pl.on_actor_lookup()]);

        // Update charges for both.
        let _ = state.charge_actor_update(&gas, 101).unwrap();
        assert_charges(&gas, [pl.on_actor_lookup(), pl.on_actor_update()]);

        // Read doesn't charge if we've already recorded a charge.
        state.record_actor_read(101);
        let _ = state.charge_actor_read(&gas, 101).unwrap();
        assert_charges(&gas, []);

        // Update still charges for the write.
        let _ = state.charge_actor_update(&gas, 101).unwrap();
        assert_charges(&gas, [pl.on_actor_update()]);

        // But not if we've already recorded the update.
        state.record_actor_update(101);
        let _ = state.charge_actor_update(&gas, 101).unwrap();
        assert_charges(&gas, []);

        // We never "downgrade" the state of the access.
        state.record_actor_read(101);
        let _ = state.charge_actor_update(&gas, 101).unwrap();
        let _ = state.charge_actor_read(&gas, 101).unwrap();
        assert_charges(&gas, []);

        // If we commit the changes, the charges should stick.
        state.end_transaction(false).unwrap();
        let _ = state.charge_actor_read(&gas, 101).unwrap();
        assert_charges(&gas, []);

        // Ending a transaction while none is ongoing should fail.
        state.end_transaction(false).unwrap_err();

        // Make sure we charge
        state.record_actor_read(101);
        state.begin_transaction();
        state.record_actor_read(102);
        // Shouldn't charge because we've already recorded the access.
        let _ = state.charge_actor_read(&gas, 101).unwrap();
        let _ = state.charge_actor_read(&gas, 102).unwrap();
        assert_charges(&gas, []);

        // Should "unrecord" the read of 102, but not the read of one.
        state.end_transaction(true).unwrap();
        let _ = state.charge_actor_read(&gas, 101).unwrap();
        assert_charges(&gas, []);
        let _ = state.charge_actor_read(&gas, 102).unwrap();
        assert_charges(&gas, [pl.on_actor_lookup()]);
    }

    #[test]
    fn test_state_access_tracker_actor_free() {
        let (_, state, gas) = new_tracker();

        // We shouldn't charge for actors in the "preload" list.
        let _ = state.charge_actor_read(&gas, 1).unwrap();
        let _ = state.charge_actor_update(&gas, 1).unwrap();
        assert_charges(&gas, []);
    }

    #[test]
    fn test_state_access_tracker_lookup() {
        let (pl, state, gas) = new_tracker();

        // Never charge to lookup ID addresses.
        let _ = state
            .charge_address_lookup(&gas, &Address::new_id(1))
            .unwrap();
        assert_charges(&gas, []);

        // Charge for address lookup.
        let t_addr = Address::new_secp256k1(&[0; SECP_PUB_LEN][..]).unwrap();
        let _ = state.charge_address_lookup(&gas, &t_addr).unwrap();
        assert_charges(&gas, [pl.on_resolve_address()]);

        // But only if we haven't already.
        state.record_lookup_address(&t_addr);
        assert_charges(&gas, []);
    }
}
