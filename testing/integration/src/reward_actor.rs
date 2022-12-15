use fvm_ipld_encoding::repr::*;
use fvm_ipld_encoding::tuple::*;
use fvm_ipld_encoding::Cbor;
use fvm_shared::bigint::bigint_ser;
use fvm_shared::clock::{ChainEpoch, EPOCH_UNDEFINED};
use fvm_shared::econ::TokenAmount;

use fvm_shared::sector::{Spacetime, StoragePower};
use fvm_shared::smooth::{AlphaBetaFilter, FilterEstimate, DEFAULT_ALPHA, DEFAULT_BETA};
use lazy_static::lazy_static;
use fvm_shared::math::PRECISION;
use std::str::FromStr;

lazy_static! {
    pub static ref BASELINE_EXPONENT: StoragePower =
        StoragePower::from_str("340282591298641078465964189926313473653").unwrap();
    /// 36.266260308195979333 FIL
    pub static ref INITIAL_REWARD_POSITION_ESTIMATE: TokenAmount = TokenAmount::from_atto(36266260308195979333u128);
    /// -1.0982489*10^-7 FIL per epoch.  Change of simple minted tokens between epochs 0 and 1.
    pub static ref INITIAL_REWARD_VELOCITY_ESTIMATE: TokenAmount = TokenAmount::from_atto(-109897758509i64);

    pub static ref BASELINE_INITIAL_VALUE: StoragePower = StoragePower::from(2_888_888_880_000_000_000u128);

    /// 1EiB
        pub static ref INIT_BASELINE_POWER: StoragePower =
        ((BASELINE_INITIAL_VALUE.clone() << (2*PRECISION)) / &*BASELINE_EXPONENT) >> PRECISION;

    /// 330M for mainnet
    pub(super) static ref SIMPLE_TOTAL: TokenAmount = TokenAmount::from_whole(330_000_000);
    /// 770M for mainnet
    pub(super) static ref BASELINE_TOTAL: TokenAmount = TokenAmount::from_whole(770_000_000);
}

/// Reward actor state
#[derive(Serialize_tuple, Deserialize_tuple, Default, Debug, Clone)]
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
    pub this_epoch_reward: TokenAmount,
    /// Smoothed `this_epoch_reward`.
    pub this_epoch_reward_smoothed: FilterEstimate,

    /// The baseline power the network is targeting at st.Epoch.
    #[serde(with = "bigint_ser")]
    pub this_epoch_baseline_power: StoragePower,

    /// Epoch tracks for which epoch the Reward was computed.
    pub epoch: ChainEpoch,

    // TotalStoragePowerReward tracks the total FIL awarded to block miners
    pub total_storage_power_reward: TokenAmount,

    // Simple and Baseline totals are constants used for computing rewards.
    // They are on chain because of a historical fix resetting baseline value
    // in a way that depended on the history leading immediately up to the
    // migration fixing the value.  These values can be moved from state back
    // into a code constant in a subsequent upgrade.
    pub simple_total: TokenAmount,
    pub baseline_total: TokenAmount,
}

impl State {
    pub fn new_test() -> Self {
        let mut st = Self {
            effective_baseline_power: BASELINE_INITIAL_VALUE.clone(),
            this_epoch_baseline_power: INIT_BASELINE_POWER.clone(),
            epoch: EPOCH_UNDEFINED,
            this_epoch_reward_smoothed: FilterEstimate::new(
                INITIAL_REWARD_POSITION_ESTIMATE.atto().clone(),
                INITIAL_REWARD_VELOCITY_ESTIMATE.atto().clone(),
            ),
            simple_total: SIMPLE_TOTAL.clone(),
            baseline_total: BASELINE_TOTAL.clone(),
            ..Default::default()
        };

        st
    }
}