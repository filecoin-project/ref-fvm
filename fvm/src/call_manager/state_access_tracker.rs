// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::cell::RefCell;
use std::iter;

use anyhow::Context;
use fvm_shared::address::{Address, Protocol};
use fvm_shared::ActorID;

use crate::history_map::HistoryMap;
use crate::kernel::{ClassifyResult, Result};

struct StateAccessLayer {
    addresses_height: usize,
    actors_height: usize,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, PartialOrd, Ord)]
pub enum ActorAccessState {
    Read,
    Updated,
}

pub struct StateAccessTracker {
    actors: RefCell<HistoryMap<ActorID, ActorAccessState>>,
    addresses: RefCell<HistoryMap<Address, ()>>,
    layers: Vec<StateAccessLayer>,
}

impl StateAccessTracker {
    /// Create a new state access tracker.
    pub fn new(preload_actors: &[ActorID]) -> Self {
        Self {
            actors: RefCell::new(
                iter::zip(
                    preload_actors.iter().copied(),
                    iter::repeat(ActorAccessState::Updated),
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
            self.addresses.get_mut().rollback(layer.addresses_height);
        }
        Ok(())
    }

    /// Returns the highest state-access type that has been charged for this actor.
    pub fn get_actor_access_state(&self, actor: ActorID) -> Option<ActorAccessState> {
        self.actors.borrow().get(&actor).copied()
    }

    /// Record that an actor's state was successfully read so that we don't charge for it again.
    pub fn record_actor_read(&self, actor: ActorID) {
        let mut actors = self.actors.borrow_mut();
        if actors.get(&actor).is_none() {
            actors.insert(actor, ActorAccessState::Read)
        }
    }

    /// Record that an actor's state was successfully updated so that we don't charge for it again.
    pub fn record_actor_update(&self, actor: ActorID) {
        self.actors
            .borrow_mut()
            .insert(actor, ActorAccessState::Updated)
    }

    /// Returns true if the address lookup has already been charged.
    pub fn get_address_lookup_state(&self, addr: &Address) -> bool {
        addr.protocol() == Protocol::ID || self.addresses.borrow_mut().get(addr).is_some()
    }

    /// Record that an actor's state was successfully resolved so that we don't charge for it again.
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

    use super::StateAccessTracker;
    use crate::call_manager::state_access_tracker::ActorAccessState;

    #[test]
    fn test_state_access_tracker_actor() {
        let mut state = StateAccessTracker::new(&[]);
        state.begin_transaction();

        assert_eq!(state.get_actor_access_state(101), None);
        state.record_actor_read(101);
        assert_eq!(
            state.get_actor_access_state(101),
            Some(ActorAccessState::Read)
        );
        state.record_actor_update(101);
        assert_eq!(
            state.get_actor_access_state(101),
            Some(ActorAccessState::Updated)
        );
        state.record_actor_read(101);
        assert_eq!(
            state.get_actor_access_state(101),
            Some(ActorAccessState::Updated)
        );

        // If we commit the changes, the charges should stick.
        state.end_transaction(false).unwrap();
        assert_eq!(
            state.get_actor_access_state(101),
            Some(ActorAccessState::Updated)
        );

        // Ending a transaction while none is ongoing should fail.
        state.end_transaction(false).unwrap_err();

        state.begin_transaction();
        state.record_actor_read(102);
        // Shouldn't charge because we've already recorded the access.
        assert_eq!(
            state.get_actor_access_state(101),
            Some(ActorAccessState::Updated)
        );
        assert_eq!(
            state.get_actor_access_state(102),
            Some(ActorAccessState::Read)
        );

        // Should "unrecord" the read of 102, but not the read of one.
        state.end_transaction(true).unwrap();
        assert_eq!(
            state.get_actor_access_state(101),
            Some(ActorAccessState::Updated)
        );
        assert_eq!(state.get_actor_access_state(102), None);
    }

    #[test]
    fn test_state_access_tracker_actor_free() {
        let state = StateAccessTracker::new(&[1]);

        // We shouldn't charge for actors in the "preload" list.
        assert_eq!(
            state.get_actor_access_state(1),
            Some(ActorAccessState::Updated)
        );
    }

    #[test]
    fn test_state_access_tracker_lookup() {
        let mut state = StateAccessTracker::new(&[]);

        // Never charge to lookup ID addresses.
        assert!(state.get_address_lookup_state(&Address::new_id(1)));

        state.begin_transaction();

        let t_addr = Address::new_secp256k1(&[0; SECP_PUB_LEN][..]).unwrap();
        assert!(!state.get_address_lookup_state(&t_addr));
        state.record_lookup_address(&t_addr);
        assert!(state.get_address_lookup_state(&t_addr));

        // Charge again if we revert.
        state.end_transaction(true).unwrap();
        assert!(!state.get_address_lookup_state(&t_addr));

        // But not if we commit it.
        state.begin_transaction();
        state.record_lookup_address(&t_addr);
        state.end_transaction(false).unwrap();
        assert!(state.get_address_lookup_state(&t_addr));
    }
}
