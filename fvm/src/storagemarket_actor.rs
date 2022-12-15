use std::str::FromStr;

use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::{BytesSer, Cbor};
use fvm_shared::clock::{ChainEpoch};
use fvm_shared::deal::DealID;
use fvm_shared::econ::TokenAmount;
use fvm_ipld_amt::Amt;
use fvm_ipld_hamt::Hamt;
use fvm_shared::piece::PaddedPieceSize;
use libipld_core::ipld::Ipld;
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use fvm_shared::address::Address;

pub type AllocationID = u64;

/// Market actor state
#[derive(Clone, Default, Serialize_tuple, Deserialize_tuple, Debug)]
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
    pub total_client_locked_collateral: TokenAmount,
    /// Total Provider Collateral that is locked -> unlocked when deal is terminated
    pub total_provider_locked_collateral: TokenAmount,
    /// Total storage fee that is locked in escrow -> unlocked when payments are made
    pub total_client_storage_fee: TokenAmount,

    /// Verified registry allocation IDs for deals that are not yet activated.
    pub pending_deal_allocation_ids: Cid, // HAMT[DealID]AllocationID
}

impl Cbor for State {}

impl State {
    // ideally we would just #[cfg(test)] this, but it is used by non test-gated code in
    // integration/tester.
    #[allow(unused)]
    pub fn new_test<B: Blockstore>(store: &B) -> Self {
        let empty_proposals_array = Amt::<(), _>::new_with_bit_width(store, 5)
            .flush()
            .unwrap();

        let empty_states_array = Amt::<(), _>::new_with_bit_width(store, 6)
            .flush()
            .unwrap();

        let empty_pending_proposals_map = Hamt::<_, ()>::new_with_bit_width(store, 5)
            .flush()
            .unwrap();
        
        let empty_balance_table = Hamt::<_,TokenAmount>::new_with_bit_width(store, 5)
            .flush()
            .unwrap();
        
        let empty_deal_ops_hamt = Hamt::<_,Cid>::new_with_bit_width(store, 5)
            .flush()
            .unwrap();
        
        let empty_pending_deal_allocation_map = Hamt::<_, AllocationID>::new_with_bit_width(store, 5)
            .flush()
            .unwrap();

        State {
            proposals: empty_proposals_array,
            states: empty_states_array,
            pending_proposals: empty_pending_proposals_map,
            escrow_table: empty_balance_table,
            locked_table: empty_balance_table,
            next_id: DealID::default(),
            deal_ops_by_epoch: empty_deal_ops_hamt,
            last_cron: ChainEpoch::default(),
            total_client_locked_collateral: TokenAmount::default(),
            total_provider_locked_collateral: TokenAmount::default(),
            total_client_storage_fee: TokenAmount::default(),
            pending_deal_allocation_ids: empty_pending_deal_allocation_map,
        }
    }

}