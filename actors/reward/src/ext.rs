use fvm_shared::{bigint::bigint_ser, econ::TokenAmount, encoding::tuple::*};

pub mod miner {
    use super::*;

    pub const APPLY_REWARDS_METHOD: u64 = 14;

    #[derive(Debug, Serialize_tuple, Deserialize_tuple)]
    pub struct ApplyRewardParams {
        #[serde(with = "bigint_ser")]
        pub reward: TokenAmount,
        #[serde(with = "bigint_ser")]
        pub penalty: TokenAmount,
    }
}
