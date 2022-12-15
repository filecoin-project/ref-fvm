use fvm_shared::bigint::bigint_ser;
use fvm_ipld_encoding::tuple::*;
use fvm_shared::clock::ChainEpoch;
use fvm_shared::econ::TokenAmount;
use fvm_shared::sector::{RegisteredPoStProof, StoragePower};
use fvm_shared::smooth::{FilterEstimate};
use cid::Cid;
use fvm_ipld_hamt::Hamt;
use fvm_ipld_blockstore::Blockstore;
use lazy_static::lazy_static;

lazy_static! {
    /// genesis power in bytes = 750,000 GiB
    pub static ref INITIAL_QA_POWER_ESTIMATE_POSITION: StoragePower = StoragePower::from(750_000) * (1 << 30);
    /// max chain throughput in bytes per epoch = 120 ProveCommits / epoch = 3,840 GiB
    pub static ref INITIAL_QA_POWER_ESTIMATE_VELOCITY: StoragePower = StoragePower::from(3_840) * (1 << 30);
}

/// Storage power actor state
#[derive(Default, Serialize_tuple, Deserialize_tuple, Clone, Debug)]
pub struct State {
    #[serde(with = "bigint_ser")]
    pub total_raw_byte_power: StoragePower,
    #[serde(with = "bigint_ser")]
    pub total_bytes_committed: StoragePower,
    #[serde(with = "bigint_ser")]
    pub total_quality_adj_power: StoragePower,
    #[serde(with = "bigint_ser")]
    pub total_qa_bytes_committed: StoragePower,
    pub total_pledge_collateral: TokenAmount,

    #[serde(with = "bigint_ser")]
    pub this_epoch_raw_byte_power: StoragePower,
    #[serde(with = "bigint_ser")]
    pub this_epoch_quality_adj_power: StoragePower,
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

impl State {
    pub fn new_test<BS: Blockstore>(store: &BS) -> Self {
        let empty_map = Hamt::<_, ()>::new_with_bit_width(store, 5)
            .flush()
            .unwrap();

        let empty_mmap = Hamt::<_, Cid>::new_with_bit_width(store, 6).flush().unwrap();
        State {
            cron_event_queue: empty_mmap,
            claims: empty_map,
            this_epoch_qa_power_smoothed: FilterEstimate::new(
                INITIAL_QA_POWER_ESTIMATE_POSITION.clone(),
                INITIAL_QA_POWER_ESTIMATE_VELOCITY.clone(),
            ),
            ..Default::default()
        }
    }
}