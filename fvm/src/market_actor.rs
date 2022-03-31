//! This module contains the types and functions to process the market actor's state.
//! This ONLY exists to support the circulating supply calc for version <= 14.
//!
//! It should be removed as soon as the Filecoin network updates to v15.

use anyhow::Context;
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::bigint::bigint_ser;
use fvm_ipld_blockstore::{Blockstore, CborStore};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::deal::DealID;
use fvm_shared::econ::TokenAmount;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::Cbor;

use crate::kernel::{ClassifyResult, Result};
use crate::state_tree::{ActorState, StateTree};

pub const MARKET_ACTOR_ADDR: Address = Address::new_id(5);

/// Market power actor state

impl Cbor for State {}
#[derive(Clone, Default, Serialize_tuple, Deserialize_tuple)]
pub struct State {
    /// Proposals are deals that have been proposed and not yet cleaned up after expiry or termination.
    /// Array<DealID, DealProposal>
    pub proposals: Cid,

    // States contains state for deals that have been activated and not yet cleaned up after expiry or termination.
    // After expiration, the state exists until the proposal is cleaned up too.
    // Invariant: keys(States) ⊆ keys(Proposals).
    /// Array<DealID, DealState>
    pub states: Cid,

    /// PendingProposals tracks dealProposals that have not yet reached their deal start date.
    /// We track them here to ensure that miners can't publish the same deal proposal twice
    pub pending_proposals: Cid,

    /// Total amount held in escrow, indexed by actor address (including both locked and unlocked amounts).
    pub escrow_table: Cid,

    /// Amount locked, indexed by actor address.
    /// Note: the amounts in this table do not affect the overall amount in escrow:
    /// only the _portion_ of the total escrow amount that is locked.
    pub locked_table: Cid,

    /// Deal id state sequential incrementer
    pub next_id: DealID,

    /// Metadata cached for efficient iteration over deals.
    /// SetMultimap<Address>
    pub deal_ops_by_epoch: Cid,
    pub last_cron: ChainEpoch,

    /// Total Client Collateral that is locked -> unlocked when deal is terminated
    #[serde(with = "bigint_ser")]
    pub total_client_locked_colateral: TokenAmount,
    /// Total Provider Collateral that is locked -> unlocked when deal is terminated
    #[serde(with = "bigint_ser")]
    pub total_provider_locked_colateral: TokenAmount,
    /// Total storage fee that is locked in escrow -> unlocked when payments are made
    #[serde(with = "bigint_ser")]
    pub total_client_storage_fee: TokenAmount,
}

impl State {
    /// Loads the market actor state with the supplied CID from the underlying store.
    pub fn load<B>(state_tree: &StateTree<B>) -> Result<(Self, ActorState)>
    where
        B: Blockstore,
    {
        let market_act = state_tree
            .get_actor(&MARKET_ACTOR_ADDR)?
            .context("Market actor address could not be resolved")
            .or_fatal()?;

        let state = state_tree
            .store()
            .get_cbor(&market_act.state)
            .or_fatal()?
            .context("market actor state not found")
            .or_fatal()?;
        Ok((state, market_act))
    }

    pub fn total_locked(&self) -> TokenAmount {
        &self.total_client_locked_colateral
            + &self.total_provider_locked_colateral
            + &self.total_client_storage_fee
    }
}
