use anyhow::Context;
use fvm_shared::bigint::bigint_ser;
use fvm_shared::blockstore::{Blockstore, CborStore};
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::encoding::tuple::*;
use fvm_shared::encoding::Cbor;
use fvm_shared::sector::{Spacetime, StoragePower};
use fvm_shared::smooth::FilterEstimate;
use fvm_shared::ActorID;

use crate::kernel::{ClassifyResult, Result};
use crate::state_tree::{ActorState, StateTree};

pub const REWARD_ACTOR_ID: ActorID = 2;

impl Cbor for State {}
/// Reward actor state
#[derive(Serialize_tuple, Deserialize_tuple, Default)]
pub struct State {
    /// Target CumsumRealized needs to reach for EffectiveNetworkTime to increase
    /// Expressed in byte-epochs.
    #[serde(with = "bigint_ser")]
    pub cumsum_baseline: Spacetime,

    /// CumsumRealized is cumulative sum of network power capped by BaselinePower(epoch).
    /// Expressed in byte-epochs.
    #[serde(with = "bigint_ser")]
    pub cumsum_realized: Spacetime,

    /// Ceiling of real effective network time `theta` based on
    /// CumsumBaselinePower(theta) == CumsumRealizedPower
    /// Theta captures the notion of how much the network has progressed in its baseline
    /// and in advancing network time.
    pub effective_network_time: ChainEpoch,

    /// EffectiveBaselinePower is the baseline power at the EffectiveNetworkTime epoch.
    #[serde(with = "bigint_ser")]
    pub effective_baseline_power: StoragePower,

    /// The reward to be paid in per WinCount to block producers.
    /// The actual reward total paid out depends on the number of winners in any round.
    /// This value is recomputed every non-null epoch and used in the next non-null epoch.
    #[serde(with = "bigint_ser")]
    pub this_epoch_reward: TokenAmount,
    /// Smoothed `this_epoch_reward`.
    pub this_epoch_reward_smoothed: FilterEstimate,

    /// The baseline power the network is targeting at st.Epoch.
    #[serde(with = "bigint_ser")]
    pub this_epoch_baseline_power: StoragePower,

    /// Epoch tracks for which epoch the Reward was computed.
    pub epoch: ChainEpoch,

    // TotalStoragePowerReward tracks the total FIL awarded to block miners
    #[serde(with = "bigint_ser")]
    pub total_storage_power_reward: TokenAmount,

    // Simple and Baseline totals are constants used for computing rewards.
    // They are on chain because of a historical fix resetting baseline value
    // in a way that depended on the history leading immediately up to the
    // migration fixing the value.  These values can be moved from state back
    // into a code constant in a subsequent upgrade.
    #[serde(with = "bigint_ser")]
    pub simple_total: TokenAmount,
    #[serde(with = "bigint_ser")]
    pub baseline_total: TokenAmount,
}

impl State {
    /// Loads the reward actor state with the supplied CID from the underlying store.
    pub fn load<B>(state_tree: &StateTree<B>) -> Result<(Self, ActorState)>
    where
        B: Blockstore,
    {
        let reward_act = state_tree
            .get_actor_id(REWARD_ACTOR_ID)?
            .context("Reward actor address could not be resolved")
            .or_fatal()?;

        let state = state_tree
            .store()
            .get_cbor(&reward_act.state)
            .or_fatal()?
            .context("reward actor state not found")
            .or_fatal()?;
        Ok((state, reward_act))
    }

    pub fn total_storage_power_reward(&self) -> TokenAmount {
        self.total_storage_power_reward.clone()
    }
}
