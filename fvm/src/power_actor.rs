//! This module contains the types and functions to process the power actor's state.
//! This ONLY exists to support the circulating supply calc for version <= 14.
//!
//! It should be removed as soon as the Filecoin network updates to v15.

use anyhow::Context;
use cid::Cid;
use fvm_shared::address::Address;
use fvm_shared::bigint::bigint_ser;
use fvm_shared::blockstore::{Blockstore, CborStore};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::tuple::*;
use fvm_shared::encoding::Cbor;
use fvm_shared::sector::StoragePower;
use fvm_shared::smooth::FilterEstimate;

use crate::kernel::{ClassifyResult, Result};
use crate::state_tree::{ActorState, StateTree};

pub const POWER_ACTOR_ADDR: Address = Address::new_id(4);

/// Storage power actor state
#[derive(Default, Serialize_tuple, Deserialize_tuple)]
pub struct State {
    #[serde(with = "bigint_ser")]
    pub total_raw_byte_power: StoragePower,
    #[serde(with = "bigint_ser")]
    pub total_bytes_committed: StoragePower,
    #[serde(with = "bigint_ser")]
    pub total_quality_adj_power: StoragePower,
    #[serde(with = "bigint_ser")]
    pub total_qa_bytes_committed: StoragePower,
    #[serde(with = "bigint_ser")]
    pub total_pledge_collateral: TokenAmount,

    #[serde(with = "bigint_ser")]
    pub this_epoch_raw_byte_power: StoragePower,
    #[serde(with = "bigint_ser")]
    pub this_epoch_quality_adj_power: StoragePower,
    #[serde(with = "bigint_ser")]
    pub this_epoch_pledge_collateral: TokenAmount,
    pub this_epoch_qa_power_smoothed: FilterEstimate,

    pub miner_count: i64,
    /// Number of miners having proven the minimum consensus power.
    pub miner_above_min_power_count: i64,

    /// A queue of events to be triggered by cron, indexed by epoch.
    pub cron_event_queue: Cid, // Multimap, (HAMT[ChainEpoch]AMT[CronEvent]

    /// First epoch in which a cron task may be stored. Cron will iterate every epoch between this
    /// and the current epoch inclusively to find tasks to execute.
    pub first_cron_epoch: ChainEpoch,

    /// Claimed power for each miner.
    pub claims: Cid, // Map, HAMT[address]Claim

    pub proof_validation_batch: Option<Cid>,
}

impl Cbor for State {}

impl State {
    /// Loads the power actor state with the supplied CID from the underlying store.
    pub fn load<B>(state_tree: &StateTree<B>) -> Result<(Self, ActorState)>
    where
        B: Blockstore,
    {
        let power_act = state_tree
            .get_actor(&POWER_ACTOR_ADDR)?
            .context("Power actor address could not be resolved")
            .or_fatal()?;

        let state = state_tree
            .store()
            .get_cbor(&power_act.state)
            .or_fatal()?
            .context("power actor state not found")
            .or_fatal()?;
        Ok((state, power_act))
    }

    pub fn total_locked(self) -> TokenAmount {
        self.total_pledge_collateral
    }
}
